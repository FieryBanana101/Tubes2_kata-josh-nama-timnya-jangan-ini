use std::sync::{Arc, Mutex};
use std::thread;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::tokenizer::{parser, Node, Element};
use crate::css_selector::*;



#[derive(Debug, Clone)]
struct ThreadTask {
    curr_node       : Arc<Element>,
    curr_node_idx   : usize,
    parent_node     : Arc<Element>,
    curr_filter_idx : usize,
    depth           : usize
}



#[derive(Debug, Clone)]
pub struct AsyncStack<T> {
    data: Arc<Mutex<Vec<T>>>,
}

impl<T> AsyncStack<T> {

    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(Vec::new())),
        }
    }


    pub fn push(&self, item: T) -> () where T: std::fmt::Debug {
        let mut vec = self.data.lock().expect("Failed to lock stack for push");
        vec.push(item);
    }


    pub fn pop(&self) -> Option<T> {
        let mut vec = self.data.lock().expect("Failed to lock stack for pop");
        vec.pop()
    }


    pub fn peek(&self) -> Option<T> 
    where  T: Clone 
    {
        let vec: std::sync::MutexGuard<'_, Vec<T>> = self.data.lock().expect("Failed to lock stack for peek");
        vec.last().cloned()
    }

    pub fn len(&self) -> usize {
        let vec = self.data.lock().expect("Failed to lock stack for len");
        vec.len()
    }

}



fn push_all_children(
    task_stack: &Arc<AsyncStack<ThreadTask>>, 
    curr_node: &Arc<Element>, 
    curr_depth: usize, 
    filter_idx: usize, 
    add_prev_filter: bool
){

    dbg!(&curr_node.tag, filter_idx);
    for (idx, node) in (&curr_node).children.iter().enumerate().rev() {

        dbg!(&curr_node.tag, &curr_node.attributes, &node, filter_idx, add_prev_filter); eprintln!("\n\n");
        if let Node::Element(child) = node {
            
            task_stack.push(ThreadTask{
                curr_node: child.clone(),
                curr_node_idx: idx,
                parent_node: curr_node.clone(),
                curr_filter_idx: filter_idx,
                depth: curr_depth + 1
            });

            if add_prev_filter {
                task_stack.push(ThreadTask{
                    curr_node: child.clone(),
                    curr_node_idx: idx,
                    parent_node: curr_node.clone(),
                    curr_filter_idx: filter_idx - 1,
                    depth: curr_depth + 1
                });
            }
            
        }

    }

}



fn push_next_sibling(
    task_stack: &Arc<AsyncStack<ThreadTask>>, 
    parent_node: &Arc<Element>, 
    curr_child_idx: usize, 
    curr_depth: usize, 
    filter_idx: usize, 
    add_prev_filter: bool
){

    let next_sibling = parent_node.children.get(curr_child_idx + 1);
    if !next_sibling.is_none() {

        let pushed_node = if let Some(Node::Element(node)) = next_sibling { node.clone() } else { unreachable!() };

        task_stack.push(ThreadTask{
            curr_node: pushed_node.clone(),
            curr_node_idx: curr_child_idx + 1,
            parent_node: parent_node.clone(),
            curr_filter_idx: filter_idx,
            depth: curr_depth
        });

        if add_prev_filter {
            task_stack.push(ThreadTask{
                curr_node: pushed_node.clone(),
                curr_node_idx: curr_child_idx + 1,
                parent_node: parent_node.clone(),
                curr_filter_idx: filter_idx,
                depth: curr_depth
            });
        }
        
    }

}

pub fn async_traversal_base(html_text: &str, css_query: &str) {

    let tree = parser(html_text).expect("Failed to parse HTML");
    dbg!(&tree); // Temporary debug
    return;
    let css_filters = CssSelectorParser::new(css_query, false).parse_all();

    let core_num = thread::available_parallelism()
        .expect("Failed to get the number of CPU cores before a pure DFS traversal")
        .get();
    let core_num = 1; // Temporary for debugging


    let async_task_stack: Arc<AsyncStack<ThreadTask>> = Arc::new(AsyncStack::<ThreadTask>::new());
    let async_filters: Arc<Vec<NodeFilter>>  = Arc::new(css_filters);

    let atomic_counter: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));

    let mut threads: Vec<thread::JoinHandle<()>> = vec![];


    async_task_stack.push(ThreadTask { 
        curr_node: Arc::clone(&tree),
        parent_node: Arc::clone(&tree),
        curr_node_idx: 0,
        curr_filter_idx: 0,
        depth: 0,
    });

    
    for i in 0..core_num {
        
        let task_stack = Arc::clone(&async_task_stack);
        let shared_filters = Arc::clone(&async_filters);

        let active_threads_count = Arc::clone(&atomic_counter);


        let thread = thread::spawn(move || {
            
            'thread_loop: loop {
                let mut task = task_stack.pop();

                while task.is_none() {
                    if active_threads_count.load(Ordering::SeqCst) == 0 {
                        break 'thread_loop;
                    }
                    task = task_stack.pop();
                }

                active_threads_count.fetch_add(1, Ordering::SeqCst);
                
                let ThreadTask{ 
                    curr_node, 
                    curr_node_idx,
                    parent_node, 
                    curr_filter_idx,
                    depth 
                } = task.unwrap();

                
                let curr_filter: &NodeFilter = shared_filters.get(curr_filter_idx).unwrap();
                let node_match_filter = curr_filter.selector.match_node(&curr_node);
                
                if node_match_filter {

                    if curr_filter_idx + 1 == shared_filters.len() {
                        eprintln!("PASSED");
                        dbg!(&curr_node, i);
                        eprintln!("\n\n");
                        active_threads_count.fetch_sub(1, Ordering::SeqCst);
                        continue 'thread_loop;
                    }

                    let next_filter = &shared_filters.get(curr_filter_idx + 1).expect("Invalid css filter index");
                    let next_combinator = next_filter.prev_combinator.as_ref().expect("Unexpected None value Combinator");
                    match next_combinator {

                        Combinator::Child if curr_filter.prev_combinator.as_ref().is_none_or(|val| *val == Combinator::Descendant) => { 
                            push_all_children(&task_stack, &curr_node, depth, curr_filter_idx + 1, true);
                        },

                        Combinator::Descendant | Combinator::Child => { 
                            push_all_children(&task_stack, &curr_node, depth, curr_filter_idx + 1, false);
                        }
                        
                        Combinator::DirectNextSibling if curr_filter.prev_combinator == Some(Combinator::NextSibling) => {
                            push_next_sibling(&task_stack, &parent_node, curr_node_idx, depth, curr_filter_idx + 1, true);
                        },
                        
                        Combinator::NextSibling | Combinator::DirectNextSibling => {
                            push_next_sibling(&task_stack, &parent_node, curr_node_idx, depth, curr_filter_idx + 1, false);
                        }
                    };

                }
                else{
                    
                    match curr_filter.prev_combinator {
                        
                        Some(Combinator::Descendant) | None => {
                            if curr_node.tag == "ul" { eprintln!("HERE"); dbg!(&curr_node, &curr_filter_idx); eprintln!("\n\n");}
                            push_all_children(&task_stack, &curr_node, depth, curr_filter_idx, false);
                        },


                        Some(Combinator::NextSibling) => {
                            push_next_sibling(&task_stack, &parent_node, curr_node_idx, depth, curr_filter_idx, false);
                        },

                        _ => {}
                    }

                }
                

                active_threads_count.fetch_sub(1, Ordering::SeqCst);

            }

        });


        threads.push(thread);
    }


    for thread in threads {
        thread.join().expect("Failed to join thread after pure DFS traversal");
    }

}




#[cfg(test)]
mod tests {

    use super::*; 

    #[test]
    fn test_dfs(){

        let html = r##"<!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <title>CSS Selector Test Bench</title>
        </head>
        <body>
            <header id="main-header" class="ui-component">
                <nav class="navigation" data-state="active">
                    <ul class="nav-list">
                        <li class="nav-item active"><a href="#home">Home</a></li>
                        <li class="nav-item"><a href="#about">About</a></li>
                        <li class="nav-item dropdown">
                            <span class="label">Services</span>
                            <div class="menu-container">
                                <ul class="sub-menu">
                                    <li class="sub-item"><a href="/web">Web Design</a></li>
                                    <li class="sub-item featured"><a href="/seo">SEO Optimization</a></li>
                                </ul>
                            </div>
                        </li>
                    </ul>
                </nav>
            </header>

            <main id="content-area">
                <section class="container" id="intro">
                    <h1 class="title">Complex DOM Tree Testing</h1>
                    <p class="description">This is a <span>nested span inside a p</span> for testing.</p>
                    
                    <div class="sibling-wrapper">
                        <h2 class="section-subtitle">Siblings</h2>
                        <p class="before">I am before the target.</p>
                        <div class="target">I am the target element.</div>
                        <p class="after">I am the immediate sibling.</p>
                        <p class="after-far">I am a distant sibling.</p>
                    </div>
                </section>

                <section class="grid-layout">
                    <div class="card row-1 col-1 primary">Card 1</div>
                    <div class="card row-1 col-2 secondary">Card 2</div>
                    <div class="card row-2 col-1 secondary">Card 3</div>
                    <div class="card row-2 col-2 primary active">Card 4</div>
                </section>

                <section class="deep-nesting-test">
                    <div class="level-1">
                        <div class="level-2">
                            <div class="level-3">
                                <div class="level-4">
                                    <article class="deep-article">
                                        <header>
                                            <h3 class="highlight">Recursive Search Target</h3>
                                        </header>
                                        <footer class="meta-data">
                                            <span class="author">Author Name</span>
                                        </footer>
                                    </article>
                                </div>
                            </div>
                        </div>
                    </div>
                </section>
            </main>

            <footer id="main-footer">
                <div class="footer-links">
                    <a href="/privacy" class="link">Privacy</a>
                    <a href="/terms" class="link">Terms</a>
                </div>
                <p class="copyright">&lt; &copy; 2026 CSS Parser Test Suite</p>
            </footer>
        </body>
        </html>"##;

        let testcases = vec![
            //r##"  body ul > li  a[href ^= "/"][href$="web"   ]  "##,
            //r##"html > body main#content-area section.deep-nesting-test div.level-1 div.level-3 > div.level-4 article.deep-article header h3.highlight"##,
            //r##"     header ~body~ footer"##,
            r##"     li ~ li > span + div li       "##,
            //r##" header.ui-component + main#content-area section.container div.sibling-wrapper h2 ~ p.before + div.target + p.after "##,
            //r##" html > body header#main-header.ui-component nav[data-state="active"] ul.nav-list > li.dropdown div.menu-container ul.sub-menu li.featured > a[href="/seo"] "##
        ];


        for (idx, selector_query) in testcases.iter().enumerate() {
            eprintln!("##### Pure DFS Test Case {} #####", idx);
            async_traversal_base(&html, &selector_query);
        }


    }
}