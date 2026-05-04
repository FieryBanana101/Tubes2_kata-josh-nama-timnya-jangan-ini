use std::collections::{VecDeque, HashSet, HashMap};
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::{Arc, Mutex, OnceLock};
use crate::html::Element;



/* Shared global variable, for the current tree stored in the heap and a mapping from its global_id to the heap pointer */
pub static CURRENT_TREE: OnceLock<Mutex<Arc<Element>>> = OnceLock::new();
pub static ID_TO_NODE_MAP: OnceLock<Mutex<HashMap<usize, Arc<Element>>>> = OnceLock::new();


pub fn get_current_tree() -> &'static Mutex<Arc<Element>> {
    CURRENT_TREE.get_or_init(|| {
        Mutex::new(
            Arc::new(Element { global_id: 0, tag: "".to_string(), attributes: HashMap::new(), children: Vec::new()})
        )
    })
}


pub fn set_id_to_node_mapping(global_id: usize, element: Arc<Element>) {
    let mut id_to_node_map =ID_TO_NODE_MAP.get_or_init(|| {
        Mutex::new(
            HashMap::new()
        )
    }).lock().unwrap();
    
    id_to_node_map.insert(global_id, element.clone());
}


pub fn id_to_node(global_id: usize) -> Arc<Element> {
    let id_to_node_map = ID_TO_NODE_MAP.get().unwrap().lock().unwrap();
    id_to_node_map[&global_id].clone()
}


/*
    Task descriptor to describe what a thread should do during one execution unit in a tree traverseal context.
*/
#[derive(Debug, Clone)]
pub struct ThreadTask {
    pub curr_node           : Arc<Element>,
    pub node_child_pos      : usize,
    pub parent_node         : Arc<Element>,
    pub selector_list_idx   : usize,
    pub selector_unit_idx   : usize,
    pub depth               : usize
}



/*
    Thread safe vector for various usage in asyncrhonous traversal
*/
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



/*
    Thread safe Hash Table for various usage in asyncrhonous traversal
*/
#[derive(Debug, Clone)]
pub struct AsyncHashSet<T> {
    pub data: Arc<Mutex<HashSet<T>>>,
}


impl<T> AsyncHashSet<T> {

    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashSet::new())),
        }
    }


    pub fn insert(&self, item: T)  -> bool 
    where T: Hash + Eq 
    {
        let mut set = self.data.lock().expect("Failed to lock hash set for insertion");
        set.insert(item)
    }

}


/*
    General trait for tree traversal data structure which will be the global task pool, 
    accessed by each thread to get a new task then execute it in parallel.
*/
pub trait AsyncTraversalTracker<T> {
    fn new() -> Self where Self: Sized; 
    fn push(&self, item: T) where T: Debug;
    fn pop(&self) -> Option<T>;
    fn len(&self) -> usize;
}


/*
    Thread safe stack data structure, implements the AsyncTraversalTracker trait and will be used in DFS related traversal.
*/
#[derive(Debug, Clone)]
pub struct AsyncStack<T> {
    pub data: Arc<Mutex<VecDeque<T>>>,
}


/*
    Thread safe queue data structure, implements the AsyncTraversalTracker trait and will be used in BFS related traversal.
*/
#[derive(Debug, Clone)]
pub struct AsyncQueue<T> {
    pub data: Arc<Mutex<VecDeque<T>>>,
}


/* Trait implementation for thread safe stack */
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


/* Trait implementation for thread safe queue */
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
