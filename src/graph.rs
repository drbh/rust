// -*- tab-width: 2 -*-

extern crate libc;
extern crate tensorflow_sys as tf;

use libc::c_float;
use libc::c_int;
use libc::c_uchar;
use libc::c_void;
use libc::size_t;
use std;
use std::ffi::CStr;
use std::ffi::CString;
use std::ffi::NulError;
use std::str::Utf8Error;
use std::sync::Arc;
use super::Buffer;
use super::DataType;
use super::GraphTrait;
use super::NodeTrait;
use super::Status;
use super::Result;
use super::Tensor;
use super::TensorType;

#[derive(Debug)]
struct GraphLifetime;

#[derive(Debug)]
struct GraphImpl {
  inner: *mut tf::TF_Graph,
}

impl Drop for GraphImpl {
  /// Graph will be deleted once no more Sessions are referencing it.
  fn drop(&mut self) {
    unsafe {
      tf::TF_DeleteGraph(self.inner);
    }
  }
}

////////////////////////

/// Represents a computation graph.  Graphs may be shared between sessions.
/// Graphs are thread-safe when used as directed.
#[derive(Debug)]
pub struct Graph {
  gimpl: Arc<GraphImpl>,
  lifetime: GraphLifetime,
}

impl Graph {
  /// Creates a new graph.
  pub fn new() -> Graph {
    unsafe {
      Graph {
        gimpl: Arc::new(GraphImpl{
          inner: tf::TF_NewGraph(),
        }),
        lifetime: GraphLifetime,
      }
    }
  }

  /// Node will only be added to graph when finish_node() is called
  /// (assuming finish_node() does not return an error).  graph must
  /// not be deleted until after finish_node() is called.
  pub fn new_node(&mut self, op_type: &str, node_name: &str) -> std::result::Result<NodeDescription, NulError> {
    let c_op_type = try!(CString::new(op_type));
    let c_node_name = try!(CString::new(node_name));
    unsafe {
      Ok(NodeDescription{
        inner: tf::TF_NewNode(self.gimpl.inner, c_op_type.as_ptr(), c_node_name.as_ptr()),
        graph: self,
        finished: false,
      })
    }
  }

  pub fn node_by_name(&self, node_name: &str) -> std::result::Result<Option<Node>, NulError> {
    let c_node_name = try!(CString::new(node_name));
    unsafe {
      let node = tf::TF_GraphNodeByName(self.gimpl.inner, c_node_name.as_ptr());
      if node.is_null() {
        Ok(None)
      } else {
        Ok(Some(Node {
          inner: node,
          gimpl: self.gimpl.clone(),
        }))
      }
    }
  }

  pub fn node_iter<'a>(&'a self) -> NodeIter<'a> {
    NodeIter {
      graph: &self,
      pos: 0,
    }
  }

  pub fn graph_def(&self) -> Result<Buffer<u8>> {
    let status = Status::new();
    unsafe {
      let c_buffer = tf::TF_NewBuffer();
      tf::TF_GraphToGraphDef(self.gimpl.inner, c_buffer, status.inner);
      if status.is_ok() {
        Ok(Buffer::from_c(c_buffer, true))
      } else {
        tf::TF_DeleteBuffer(c_buffer);
        Err(status)
      }
    }
  }
}

impl GraphTrait for Graph {
  fn inner(&self) -> *mut tf::TF_Graph {
    self.gimpl.inner
  }
}

////////////////////////

pub struct NodeIter<'a> {
  // We could just have a gimpl field, but keeping a reference to the Graph
  // means that the graph can't be modified while iterating through it.
  graph: &'a Graph,
  pos: size_t,
}

impl<'a> Iterator for NodeIter<'a> {
  type Item = Node;

  fn next(&mut self) -> Option<Self::Item> {
    unsafe {
      let node = tf::TF_GraphNextNode(self.graph.gimpl.inner, &mut self.pos as *mut size_t);
      if node.is_null() {
        None
      } else {
        Some(Node {
          inner: node,
          gimpl: self.graph.gimpl.clone(),
        })
      }
    }
  }
}

////////////////////////

#[derive(Debug)]
pub struct Node {
  inner: *mut tf::TF_Node,
  gimpl: Arc<GraphImpl>,
}

impl Node {
  pub fn name(&self) -> std::result::Result<String, Utf8Error> {
    unsafe {
      CStr::from_ptr(tf::TF_NodeName(self.inner)).to_str().map(|x| x.to_string())
    }
  }

  pub fn op_type(&self) -> std::result::Result<String, Utf8Error> {
    unsafe {
      CStr::from_ptr(tf::TF_NodeOpType(self.inner)).to_str().map(|x| x.to_string())
    }
  }

  pub fn device(&self) -> std::result::Result<String, Utf8Error> {
    unsafe {
      CStr::from_ptr(tf::TF_NodeOpType(self.inner)).to_str().map(|x| x.to_string())
    }
  }

  pub fn num_outputs(&self) -> usize {
    unsafe {
      tf::TF_NodeNumOutputs(self.inner) as usize
    }
  }

  pub fn output_type(&self, index: usize) -> DataType {
    unsafe {
      DataType::from_c(tf::TF_NodeOutputType(tf::TF_Port{node: self.inner, index: index as c_int}))
    }
  }

  pub fn output_list_length(&self, arg_name: &str) -> Result<usize> {
    let c_arg_name = try!(CString::new(arg_name));
    let status = Status::new();
    let length = unsafe {
      tf::TF_NodeOutputListLength(self.inner, c_arg_name.as_ptr(), status.inner)
    };
    if status.is_ok() {
      Ok(length as usize)
    } else {
      Err(status)
    }
  }

  pub fn num_inputs(&self) -> usize {
    unsafe {
      tf::TF_NodeNumInputs(self.inner) as usize
    }
  }

  pub fn input_type(&self, index: usize) -> DataType {
    unsafe {
      DataType::from_c(tf::TF_NodeInputType(tf::TF_Port{node: self.inner, index: index as c_int}))
    }
  }

  pub fn input_list_length(&self, arg_name: &str) -> Result<usize> {
    let c_arg_name = try!(CString::new(arg_name));
    let status = Status::new();
    let length = unsafe {
      tf::TF_NodeInputListLength(self.inner, c_arg_name.as_ptr(), status.inner)
    };
    if status.is_ok() {
      Ok(length as usize)
    } else {
      Err(status)
    }
  }

  pub fn input(&self, index: usize) -> (Node, usize) {
    unsafe {
      let port = tf::TF_NodeInput(tf::TF_Port{node: self.inner, index: index as c_int});
      (Node {
        inner: port.node,
        gimpl: self.gimpl.clone(),
      }, port.index as usize)
    }
  }

  pub fn output_num_consumers(&self, index: usize) -> usize {
    unsafe {
      tf::TF_NodeOutputNumConsumers(tf::TF_Port{node: self.inner, index: index as c_int}) as usize
    }
  }

  pub fn output_consumers(&self, index: usize) -> Vec<(Node, usize)> {
    unsafe {
      let num_consumers = tf::TF_NodeOutputNumConsumers(tf::TF_Port{node: self.inner, index: index as c_int});
      let mut vec = <Vec<tf::TF_Port>>::with_capacity(num_consumers as usize);
      let len = tf::TF_NodeOutputConsumers(
        tf::TF_Port{node: self.inner, index: index as c_int},
        vec.as_mut_ptr(),
        vec.len() as c_int);
      vec.set_len(len as usize);
      vec.into_iter().map(
        |port| (Node {inner: port.node, gimpl: self.gimpl.clone()}, port.index as usize)
          ).collect()
    }
  }

  pub fn num_control_inputs(&self) -> usize {
    unsafe {
      tf::TF_NodeNumControlInputs(self.inner) as usize
    }
  }

  pub fn control_inputs(&self) -> Vec<Node> {
    unsafe {
      let num_consumers = tf::TF_NodeNumControlInputs(self.inner);
      let mut vec = <Vec<*mut tf::TF_Node>>::with_capacity(num_consumers as usize);
      let len = tf::TF_NodeGetControlInputs(
        self.inner,
        vec.as_mut_ptr(),
        vec.len() as c_int);
      vec.set_len(len as usize);
      vec.into_iter().map(
        |node| Node {inner: node, gimpl: self.gimpl.clone()}
          ).collect()
    }
  }

  pub fn num_control_outputs(&self) -> usize {
    unsafe {
      tf::TF_NodeNumControlOutputs(self.inner) as usize
    }
  }

  pub fn control_outputs(&self) -> Vec<Node> {
    unsafe {
      let num_consumers = tf::TF_NodeNumControlOutputs(self.inner);
      let mut vec = <Vec<*mut tf::TF_Node>>::with_capacity(num_consumers as usize);
      let len = tf::TF_NodeGetControlOutputs(
        self.inner,
        vec.as_mut_ptr(),
        vec.len() as c_int);
      vec.set_len(len as usize);
      vec.into_iter().map(
        |node| Node {inner: node, gimpl: self.gimpl.clone()}
          ).collect()
    }
  }

  pub fn attr_value_proto(&self, attr_name: &str) -> Result<Buffer<u8>> {
    let c_attr_name = try!(CString::new(attr_name));
    unsafe {
      let status = Status::new();
      let buffer = tf::TF_NewBuffer();
      tf::TF_NodeGetAttrValueProto(
        self.inner,
        c_attr_name.as_ptr(),
        buffer,
        status.inner);
      if !status.is_ok() {
        tf::TF_DeleteBuffer(buffer);
        Err(status)
      } else {
        Ok(Buffer::from_c(buffer, true))
      }
    }
  }

  pub fn node_def(&self) -> Result<Buffer<u8>> {
    let status = Status::new();
    unsafe {
      let c_buffer = tf::TF_NewBuffer();
      tf::TF_NodeToNodeDef(self.inner, c_buffer, status.inner);
      if status.is_ok() {
        Ok(Buffer::from_c(c_buffer, true))
      } else {
        tf::TF_DeleteBuffer(c_buffer);
        Err(status)
      }
    }
  }
}

impl NodeTrait for Node {
  fn inner(&self) -> *mut tf::TF_Node {
    self.inner
  }
}

////////////////////////

#[derive(Debug,Copy,Clone)]
pub struct Port<'a> {
  pub node: &'a Node,
  pub index: c_int,
}

impl<'a> Port<'a> {
  fn to_c(&self) -> tf::TF_Port {
    tf::TF_Port {
      node: self.node.inner,
      index: self.index,
    }
  }
}

////////////////////////

#[derive(Debug)]
pub struct NodeDescription<'a> {
  inner: *mut tf::TF_NodeDescription,
  // This keeps self from outliving the Graph, which is required by the docs on TF_NewNode.
  graph: &'a Graph,
  finished: bool,
}

impl<'a> NodeDescription<'a> {
  pub fn finish(mut self) -> Result<Node> {
    self.finished = true; // used by the drop code
    let status = Status::new();
    let node = unsafe {
      tf::TF_FinishNode(self.inner, status.inner)
    };
    if status.is_ok() {
      Ok(Node{
        inner: node,
        gimpl: self.graph.gimpl.clone(),
      })
    } else {
      Err(status)
    }
  }

  pub fn set_device(&mut self, device: &str) -> std::result::Result<(), NulError> {
    let c_device = try!(CString::new(device));
    unsafe {
      tf::TF_SetDevice(self.inner, c_device.as_ptr());
    }
    Ok(())
  }

  pub fn add_input(&mut self, input: Port) {
    unsafe {
      tf::TF_AddInput(self.inner, input.to_c());
    }
  }

  pub fn add_input_list(&mut self, inputs: &[Port]) {
    let c_inputs: Vec<tf::TF_Port> = inputs.iter().map(|x| x.to_c()).collect();
    unsafe {
      tf::TF_AddInputList(self.inner, c_inputs.as_ptr(), c_inputs.len() as c_int);
    }
  }

  pub fn add_control_input(&mut self, input: &Node) {
    unsafe {
      tf::TF_AddControlInput(self.inner, input.inner);
    }
  }

  pub fn set_attr_string(&mut self, attr_name: &str, value: &str) -> std::result::Result<(), NulError> {
    let c_attr_name = try!(CString::new(attr_name));
    let c_value = value.as_bytes();
    unsafe {
      tf::TF_SetAttrString(
        self.inner,
        c_attr_name.as_ptr(),
        c_value.as_ptr() as *const c_void,
        c_value.len() as c_int);
    }
    Ok(())
  }

  pub fn set_attr_string_list<S: AsRef<str>>(&mut self, attr_name: &str, value: &[S]) -> std::result::Result<(), NulError> {
    let c_attr_name = try!(CString::new(attr_name));
    let bytes: Vec<&[u8]> = value.iter().map(|x| x.as_ref().as_bytes()).collect();
    let ptrs: Vec<*const c_void> = bytes.iter().map(|x| x.as_ptr() as *const c_void).collect();
    let lens: Vec<c_int> = bytes.iter().map(|x| x.len() as c_int).collect();
    unsafe {
      tf::TF_SetAttrStringList(
        self.inner,
        c_attr_name.as_ptr(),
        ptrs.as_ptr() as *const *const c_void,
        lens.as_ptr(),
        ptrs.len() as c_int);
    }
    Ok(())
  }

  pub fn set_attr_int(&mut self, attr_name: &str, value: i64) -> std::result::Result<(), NulError> {
    let c_attr_name = try!(CString::new(attr_name));
    unsafe {
      tf::TF_SetAttrInt(self.inner, c_attr_name.as_ptr(), value);
    }
    Ok(())
  }

  pub fn set_attr_int_list(&mut self, attr_name: &str, value: &[i64]) -> std::result::Result<(), NulError> {
    let c_attr_name = try!(CString::new(attr_name));
    unsafe {
      tf::TF_SetAttrIntList(self.inner, c_attr_name.as_ptr(), value.as_ptr(), value.len() as i32);
    }
    Ok(())
  }

  pub fn set_attr_float(&mut self, attr_name: &str, value: f32) -> std::result::Result<(), NulError> {
    let c_attr_name = try!(CString::new(attr_name));
    unsafe {
      tf::TF_SetAttrFloat(self.inner, c_attr_name.as_ptr(), value);
    }
    Ok(())
  }

  pub fn set_attr_float_list(&mut self, attr_name: &str, value: &[f32]) -> std::result::Result<(), NulError> {
    let c_attr_name = try!(CString::new(attr_name));
    let c_value: Vec<c_float> = value.iter().map(|x| *x as c_float).collect();
    unsafe {
      tf::TF_SetAttrFloatList(self.inner, c_attr_name.as_ptr(), c_value.as_ptr(), c_value.len() as i32);
    }
    Ok(())
  }

  pub fn set_attr_bool(&mut self, attr_name: &str, value: bool) -> std::result::Result<(), NulError> {
    let c_attr_name = try!(CString::new(attr_name));
    unsafe {
      tf::TF_SetAttrBool(self.inner, c_attr_name.as_ptr(), if value {1} else {0});
    }
    Ok(())
  }

  pub fn set_attr_bool_list(&mut self, attr_name: &str, value: &[bool]) -> std::result::Result<(), NulError> {
    let c_attr_name = try!(CString::new(attr_name));
    let c_value: Vec<c_uchar> = value.iter().map(|x| if *x {1} else {0}).collect();
    unsafe {
      tf::TF_SetAttrBoolList(self.inner, c_attr_name.as_ptr(), c_value.as_ptr(), c_value.len() as c_int);
    }
    Ok(())
  }

  pub fn set_attr_type(&mut self, attr_name: &str, value: DataType) -> std::result::Result<(), NulError> {
    let c_attr_name = try!(CString::new(attr_name));
    unsafe {
      tf::TF_SetAttrType(self.inner, c_attr_name.as_ptr(), value.to_c());
    }
    Ok(())
  }

  pub fn set_attr_type_list(&mut self, attr_name: &str, value: &[DataType]) -> std::result::Result<(), NulError> {
    let c_attr_name = try!(CString::new(attr_name));
    let c_value: Vec<tf::TF_DataType> = value.iter().map(|x| x.to_c()).collect();
    unsafe {
      tf::TF_SetAttrTypeList(self.inner, c_attr_name.as_ptr(), c_value.as_ptr(), c_value.len() as i32);
    }
    Ok(())
  }

  pub fn set_attr_shape(&mut self, attr_name: &str, value: &[i64]) -> std::result::Result<(), NulError> {
    let c_attr_name = try!(CString::new(attr_name));
    unsafe {
      tf::TF_SetAttrShape(self.inner, c_attr_name.as_ptr(), value.as_ptr(), value.len() as i32);
    }
    Ok(())
  }

  pub fn set_attr_shape_list<T: AsRef<[i64]>>(&mut self, attr_name: &str, value: &[T]) -> std::result::Result<(), NulError> {
    let c_attr_name = try!(CString::new(attr_name));
    let ptrs: Vec<*const i64> = value.iter().map(|x| x.as_ref().as_ptr()).collect();
    let lens: Vec<c_int> = value.iter().map(|x| x.as_ref().len() as c_int).collect();
    unsafe {
      tf::TF_SetAttrShapeList(
        self.inner,
        c_attr_name.as_ptr(),
        ptrs.as_ptr(),
        lens.as_ptr(),
        ptrs.len() as c_int);
    }
    Ok(())
  }

  pub fn set_attr_tensor_shape_proto(&mut self, attr_name: &str, value: &[u8]) -> Result<()> {
    let c_attr_name = try!(CString::new(attr_name));
    let status = Status::new();
    unsafe {
      tf::TF_SetAttrTensorShapeProto(
        self.inner,
        c_attr_name.as_ptr(),
        value.as_ptr() as *const c_void,
        value.len() as c_int,
        status.inner);
    }
    status.as_result()
  }

  pub fn set_attr_tensor_shape_proto_list<T: AsRef<[u8]>>(&mut self, attr_name: &str, value: &[T]) -> Result<()> {
    let c_attr_name = try!(CString::new(attr_name));
    let ptrs: Vec<*const c_void> = value.iter().map(|x| x.as_ref().as_ptr() as *const c_void).collect();
    let lens: Vec<c_int> = value.iter().map(|x| x.as_ref().len() as c_int).collect();
    let status = Status::new();
    unsafe {
      tf::TF_SetAttrTensorShapeProtoList(
        self.inner,
        c_attr_name.as_ptr(),
        ptrs.as_ptr(),
        lens.as_ptr(),
        ptrs.len() as c_int,
        status.inner);
    }
    status.as_result()
  }

  pub fn set_attr_tensor<T: TensorType>(&mut self, attr_name: &str, value: Tensor<T>) -> Result<()> {
    let c_attr_name = try!(CString::new(attr_name));
    let status = Status::new();
    unsafe {
      tf::TF_SetAttrTensor(
        self.inner,
        c_attr_name.as_ptr(),
        value.into_ptr(),
        status.inner);
    }
    status.as_result()
  }

  pub fn set_attr_tensor_list<T: IntoIterator<Item=Tensor<u8>>>(&mut self, attr_name: &str, value: T) -> Result<()> {
    let c_attr_name = try!(CString::new(attr_name));
    let status = Status::new();
    unsafe {
      let ptrs: Vec<*mut tf::TF_Tensor> = value.into_iter().map(|x| x.into_ptr()).collect();
      tf::TF_SetAttrTensorList(
        self.inner,
        c_attr_name.as_ptr(),
        ptrs.as_ptr(),
        ptrs.len() as c_int,
        status.inner);
    }
    status.as_result()
  }

  pub fn set_attr_to_attr_value_proto(&mut self, attr_name: &str, value: &[u8]) -> Result<()> {
    let c_attr_name = try!(CString::new(attr_name));
    let status = Status::new();
    unsafe {
      tf::TF_SetAttrToAttrValueProto(
        self.inner,
        c_attr_name.as_ptr(),
        value.as_ptr() as *const c_void,
        value.len() as size_t,
        status.inner);
    }
    status.as_result()
  }
}

impl<'a> Drop for NodeDescription<'a> {
  fn drop(&mut self) {
    if !self.finished {
      unsafe {
        // TF_NewNode requires us to make sure TF_FinishNode is called before the
        // graph is deleted.  Combined with guaranteeing that NodeDescription does
        // not outlive Graph, this ensures that the contract is held.
        let status = tf::TF_NewStatus();
        tf::TF_FinishNode(self.inner, status);
        tf::TF_DeleteStatus(status);
      }
    }
  }
}

////////////////////////

#[cfg(test)]
mod tests {
  use super::*;
  use super::super::DataType;

  fn add_node(g: &mut Graph) {
    g.new_node("Variable", "foo").unwrap();
  }
  
  #[test]
  fn smoke() {
    let mut g = Graph::new();
    add_node(&mut g);
    let node = {
      let mut nd = g.new_node("Variable", "foo").unwrap();
      nd.set_attr_type("dtype", DataType::Float).unwrap();
      nd.set_attr_shape("shape", &vec![]).unwrap();
      nd.finish().unwrap()
    };
    let mut nd2 = g.new_node("Variable", "foo2").unwrap();
    nd2.set_attr_type("dtype", DataType::Float).unwrap();
    nd2.set_attr_shape("shape", &vec![]).unwrap();
    let node2 = nd2.finish().unwrap();
    assert_eq!("foo", node.name().unwrap());
    assert_eq!("foo2", node2.name().unwrap());
  }
}