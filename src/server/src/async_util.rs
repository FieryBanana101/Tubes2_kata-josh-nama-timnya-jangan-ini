use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use crate::tokenizer::Element;

#[derive(Debug, Clone)]
pub struct ThreadTask {
    pub curr_node       : Arc<Element>,
    pub node_child_pos   : usize,
    pub parent_node     : Arc<Element>,
    pub curr_filter_idx : usize,
    pub depth           : usize
}



#[derive(Debug, Clone)]
pub struct AsyncVec<T> {
    pub data: Arc<Mutex<Vec<T>>>,
}


impl<T> AsyncVec<T> {

    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(Vec::new())),
        }
    }


    pub fn push(&self, item: T) {
        let mut vec = self.data.lock().expect("Failed to lock queue for push");
        vec.push(item);
    }


    pub fn get_vec(self) -> Option<Vec<T>> {
        let mutex = Arc::into_inner(self.data)?;
        mutex.into_inner().ok()
    }

}



pub trait AsyncTraversalTracker<T> {
    fn new() -> Self where Self: Sized; 
    fn push(&self, item: T) where T: Debug;
    fn pop(&self) -> Option<T>;
    fn len(&self) -> usize;
}


#[derive(Debug, Clone)]
pub struct AsyncStack<T> {
    pub data: Arc<Mutex<VecDeque<T>>>,
}


#[derive(Debug, Clone)]
pub struct AsyncQueue<T> {
    pub data: Arc<Mutex<VecDeque<T>>>,
}



impl<T> AsyncTraversalTracker<T> for AsyncStack<T> {

    fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(VecDeque::new())),
        }
    }


    fn push(&self, item: T) -> () where T: std::fmt::Debug {
        let mut vec = self.data.lock().expect("Failed to lock stack for push");
        vec.push_back(item);
    }


    fn pop(&self) -> Option<T> {
        let mut vec = self.data.lock().expect("Failed to lock stack for pop");
        vec.pop_back()
    }


    fn len(&self) -> usize {
        let vec = self.data.lock().expect("Failed to lock stack for len");
        vec.len()
    }


}



impl<T> AsyncTraversalTracker<T> for AsyncQueue<T> {

    fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(VecDeque::new())),
        }
    }


    fn push(&self, item: T) -> () where T: Debug {
        let mut vec = self.data.lock().expect("Failed to lock queue for push");
        vec.push_back(item);
    }


    fn pop(&self) -> Option<T> {
        let mut vec = self.data.lock().expect("Failed to lock queue for pop");
        vec.pop_front()
    }


    fn len(&self) -> usize {
        let vec = self.data.lock().expect("Failed to lock queue for len");
        vec.len()
    }


}
