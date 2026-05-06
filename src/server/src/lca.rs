use std::sync::{Arc, OnceLock, Mutex};
use std::collections::HashMap;
use crate::async_util::id_to_node;
use crate::html::{Node, Element};



/* Convert a Node (which is either an Arc<Element> or Arc<String>) into raw pointer in usize type */
fn node_ptr_to_usize(node: Node) -> usize {
    match node {

        Node::Element(element) => {
            Arc::as_ptr(&element) as usize
        },
        
        Node::Text(text) => {
            Arc::as_ptr(&text) as usize
        },

    }
}


/* Structure describing a preprocessing result for binary lift operation on a tree */
#[derive(Debug, Clone)]
pub struct BinaryLiftMetadata {
    ancestors               : Vec<Vec<usize>>,
    max_precompute_height   : usize,
    depth_map               : Vec<usize>, 
    max_depth               : usize,
    id_to_node_map          : Vec<Node>,
    node_to_id_map          : HashMap<usize, usize>
}


pub static CURRENT_BINARY_LIFT_METADATA: OnceLock<Mutex<BinaryLiftMetadata>> = OnceLock::new();


pub fn get_current_binary_lift_metadata() -> &'static Mutex<BinaryLiftMetadata> {
    CURRENT_BINARY_LIFT_METADATA.get().unwrap()
}

pub fn init_binary_lift_metadata(root: &Node) -> usize {

    let new_metadata = preprocess_tree(root);
    let max_depth = new_metadata.max_depth;

    if let Err(_) = CURRENT_BINARY_LIFT_METADATA.set(Mutex::new(new_metadata.clone())) {
        let mutex = CURRENT_BINARY_LIFT_METADATA.get().unwrap();
        let mut guard = mutex.lock().unwrap();
        *guard = new_metadata;
    }

    max_depth
}



pub fn preprocess_tree(root: &Node) -> BinaryLiftMetadata {
    
    /* 
        First, map each node in the tree into a unique ID and also precompute all of its parent. 
        Parent of the root is defined as the root itself.
        Also we count the number of node as well.
    */

    let mut id_to_node_map: Vec<Node> = Vec::new();
    let mut node_to_id_map = HashMap::<usize, usize>::new();
    let mut node_parent_map = HashMap::<usize, usize>::new();
    let mut depth_map: Vec<usize> = Vec::new();
    let mut stack: Vec<(Node, Node, usize)> = Vec::new();


    /* Do DFS to do the afromentioned mapping */
    let mut counter = 0;
    let mut max_depth = 0;
    stack.push((root.clone(), root.clone(), 0));

    while !stack.is_empty() {

        let (curr_node, parent_node, curr_depth) = stack.pop().unwrap();

        let curr_ptr = node_ptr_to_usize(curr_node.clone());
        let parent_ptr = node_ptr_to_usize(parent_node.clone());

        node_parent_map.insert(curr_ptr, parent_ptr);
        id_to_node_map.push(curr_node.clone());
        node_to_id_map.insert(curr_ptr, counter);

        depth_map.push(curr_depth);
        if max_depth < curr_depth { max_depth = curr_depth; }

        counter += 1;

        if let Node::Element(element) = curr_node.clone() {

            for child in element.children.iter().rev() {
                stack.push((child.clone(), curr_node.clone(), curr_depth + 1));
            }

        }

    }


    /* 
        Then we calculate the ancestor table according to the classic
        binary lifting dynamic programming formula.
    */

    let n = counter;

    /* Compute max_precompute_height = ceil(log2(n)) */
    let mut max_precompute_height = 0;
    let mut temp = 1;
    while temp < n {
        temp <<= 1;
        max_precompute_height += 1;
    }
    max_precompute_height += 1;


    /* Precompute the ancestors table */
    let mut ancestors: Vec<Vec<usize>> = vec![ vec![0; max_precompute_height]; n ];
    
    for i in 0..n {
        
        let ptr = node_ptr_to_usize(id_to_node_map[i].clone());
        ancestors[i][0] = node_to_id_map[ &node_parent_map[&ptr] ];

        for j in 1..max_precompute_height {
            ancestors[i][j] = ancestors[ ancestors[i][j-1] ][j-1];
        }
    }

    BinaryLiftMetadata { ancestors, max_precompute_height, depth_map, max_depth, id_to_node_map, node_to_id_map }

}




fn add_lca_path(local_binary_lift_id: usize, id_to_node_map: &Vec<Node>, path_tracker: &mut Vec<(usize, usize)>) {
    let node = id_to_node_map[local_binary_lift_id].clone();
    if let Node::Element(element) = node {
        path_tracker.push((element.global_id, element.global_id));
    }
}


pub fn find_lca(node1: &Node, node2: &Node, metadata: &BinaryLiftMetadata) -> (Node, Vec<Vec<(usize, usize)>>, Vec<String>) {

    /* Acquire metadata from binary lift pre-processing */
    let BinaryLiftMetadata { 
        ancestors, 
        max_precompute_height,
        depth_map, 
        max_depth,
        id_to_node_map, 
        node_to_id_map 
    } = metadata;

    let mut path_tracker: Vec<Vec<(usize, usize)>> = vec![Vec::new(), Vec::new()]; // We will only use two concurrent animation for each LCA sequence
    let mut log_tracker: Vec<String> = Vec::new();


    /* Map the actual node pointer into its corresponding ID */
    let mut x = node_to_id_map[&node_ptr_to_usize(node1.clone())];
    let mut y = node_to_id_map[&node_ptr_to_usize(node2.clone())];

    add_lca_path(x, id_to_node_map, &mut path_tracker[0]);
    add_lca_path(y, id_to_node_map, &mut path_tracker[1]);

    log_tracker.push(format!("[LCA] Beginning LCA Query for nodes ({}, {})", x, y));


    /*  
        Binary lift until both are at equal depth 
        Note for front-end guy:
            If you need the actual node structure for visualization, you can do something like this inside the if statement,
                let curr_node1: Node = id_to_node_map[x].clone();
                let curr_node2: Node = id_to_node_map[y].clone();
                visualize(curr_node1);
                visualize(curr_node2);

            Note that in this specific loop, only one of the node will 'move up' (the one referred by 'x')
    */
    if depth_map[x] < depth_map[y] { std::mem::swap(&mut x, &mut y); }
    let k = depth_map[x] - depth_map[y];

    for i in (0 .. *max_precompute_height).rev() {
        if (k & (1 << i)) != 0 {
            x = ancestors[x][i];
            add_lca_path(x, id_to_node_map, &mut path_tracker[0]);
            add_lca_path(y, id_to_node_map, &mut path_tracker[0]); // Add again for the 'y' so animation align

            log_tracker.push(format!("[LCA] Moving up to node {}", x));
        }
    }


    /* Check for the possiblity that one of them is already the LCA of the other */
    if x == y {
        log_tracker.push(format!("[LCA] Found result for current LCA query: {}", x));
        return (id_to_node_map[x].clone(), path_tracker, log_tracker);
    }


    /* 
        Binary lift until the parents of both node pointer is the same, that means just above both of them is the LCA. 
        Note for front-end guy:
            If you need the actual node structure for visualization, you can do something like this inside the if statement,
                let curr_node1: Node = id_to_node_map[x].clone();
                let curr_node2: Node = id_to_node_map[y].clone();
                visualize(curr_node1);
                visualize(curr_node2);
            
            Note that in this loop, both node will 'move up'
    */
    for i in (0 .. *max_precompute_height).rev() {
        if ancestors[x][i] != ancestors[y][i] {
            x = ancestors[x][i];
            y = ancestors[y][i];
            add_lca_path(x, id_to_node_map, &mut path_tracker[0]);
            add_lca_path(y, id_to_node_map, &mut path_tracker[1]);

            log_tracker.push(format!("[LCA] Moving up to node {}", x));
            log_tracker.push(format!("[LCA] Moving up to node {}", y));
        }
    }
    
    add_lca_path(ancestors[x][0], id_to_node_map, &mut path_tracker[0]);
    log_tracker.push(format!("[LCA] Found result for current LCA query: {}", ancestors[x][0]));

    let lca_result = id_to_node_map[ancestors[x][0]].clone();

    return (lca_result, path_tracker, log_tracker);
}




#[cfg(test)]
mod tests {
    use super::*;

    fn create_el(tag: &str, children: Vec<Node>) -> Node {
        Node::Element(Arc::new(Element {
            global_id: 0,
            tag: tag.to_string(),
            children: children,
            attributes: HashMap::new()
        }))
    }


    fn create_txt(content: &str) -> Node {
        Node::Text(Arc::new(content.to_string()))
    }


    #[test]
    fn test_lca() {

        /* root -> n1 -> n2 -> n3 -> n4 -> n5 */
        let n5 = create_el("n5", vec![]);
        let n4 = create_el("n4", vec![n5.clone()]);
        let n3 = create_el("n3", vec![n4.clone()]);
        let n2 = create_el("n2", vec![n3.clone()]);
        let n1 = create_el("n1", vec![n2.clone()]);
        let root = create_el("root", vec![n1.clone()]);

        let meta = preprocess_tree(&root);
        
        /* LCA(n5, n2) = n2 */
        let (res1, _, _) = find_lca(&n5, &n2, &meta);
        if let Node::Element(el) = res1 { assert_eq!(el.tag, "n2"); }


        /*  
            root    -> txt1
                    -> div2 -> s3
                            -> s4
                    -> div1 -> s1
                            -> s2
        */
        let s1 = create_el("s1", vec![]);
        let s2 = create_el("s2", vec![]);
        let s3 = create_el("s3", vec![]);
        let s4 = create_el("s4", vec![]);
        let txt1 = create_txt("I am a sibling");
        let div1 = create_el("div1", vec![s1.clone(), s2.clone()]);
        let div2 = create_el("div2", vec![s3.clone(), s4.clone()]);
        let root2 = create_el("root", vec![div1.clone(), txt1.clone(), div2.clone()]);

        let meta2 = preprocess_tree(&root2);

        /* LCA(s1, s2) = div1 */
        let (res2, _, _) = find_lca(&s1, &s2, &meta2);
        if let Node::Element(el) = res2 { assert_eq!(el.tag, "div1"); }

        /* LCA(s1, s4) = root */
        let (res3, _, _) = find_lca(&s1, &s4, &meta2);
        if let Node::Element(el) = res3 { assert_eq!(el.tag, "root"); }

        /* LCA(txt1, s3) = root */
        let (res4, _, _) = find_lca(&s3, &txt1, &meta2);
        if let Node::Element(el) = res4 { assert_eq!(el.tag, "root"); }

        /* LCA(s4, s4) = s4 */
        let (res5, _, _) = find_lca(&s4, &s4, &meta2);
        if let Node::Element(el) = res5 { assert_eq!(el.tag, "s4"); }

        /* LCA(txt1, txt1) = txt1 */
        let (res6, _, _) = find_lca(&txt1, &txt1, &meta2);
        if let Node::Text(text) = res6 { assert_eq!(*text, "I am a sibling"); }


        /* root -> [100 children] */
        let mut children = vec![];
        for i in 0..100 {
            children.push(create_el(&format!("child{}", i), vec![]));
        }
        let root3 = create_el("root", children.clone());
        let meta3 = preprocess_tree(&root3);

        /* LCA(chilld0, child99) = root */
        let (res6, _, _) = find_lca(&children[0], &children[99], &meta3);
        if let Node::Element(el) = res6 { assert_eq!(el.tag, "root"); }


        /*  
            root -> a -> b -> c -> d
                 -> e
        */      
        let d = create_el("d", vec![]);
        let c = create_el("c", vec![d.clone()]);
        let b = create_el("b", vec![c.clone()]);
        let a = create_el("a", vec![b.clone()]);
        let e = create_el("e", vec![]);
        let root = create_el("root", vec![a.clone(), e.clone()]);

        let meta = preprocess_tree(&root);

        /*  LCA(d, e) -> root */
        let (res, _, _) = find_lca(&d, &e, &meta);
        if let Node::Element(el) = res { assert_eq!(el.tag, "root"); }
        
        /*  LCA(d, b) -> b */
        let (res2, _, _) = find_lca(&d, &b, &meta);
        if let Node::Element(el) = res2 { assert_eq!(el.tag, "b"); }

        
    }
}