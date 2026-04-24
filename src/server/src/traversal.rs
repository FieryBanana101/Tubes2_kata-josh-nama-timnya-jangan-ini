use std::sync::{Arc};
use std::marker::{Send, Sync};
use std::thread;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::tokenizer::{parser, Node, Element};
use crate::css_selector::*;
use crate::async_util::*;


/* 
    Push all children of the current node into the global task pool,
    the task which is given to the children will be pointed by filter_idx as index from the CSS Selector Unit list,
    optionally we can enter the previous task accordingly and depth can be set relative to the curr_node depth.
*/
fn push_all_children<T: AsyncTraversalTracker<ThreadTask>>(
    task_tracker: &Arc<T>,
    curr_node: &Arc<Element>, 
    curr_depth: usize, 
    filter_idx: usize, 
    add_prev_filter: bool
){

    for (idx, node) in (&curr_node).children.iter().enumerate().rev() {

        if let Node::Element(child) = node {
            
            task_tracker.push(ThreadTask{
                curr_node: child.clone(),
                node_child_pos: idx,
                parent_node: curr_node.clone(),
                curr_filter_idx: filter_idx,
                depth: curr_depth + 1
            });

            if add_prev_filter {
                task_tracker.push(ThreadTask{
                    curr_node: child.clone(),
                    node_child_pos: idx,
                    parent_node: curr_node.clone(),
                    curr_filter_idx: filter_idx - 1,
                    depth: curr_depth + 1
                });
            }
            
        }

    }

}



/*
    Push the next sibling from a certain DOM Node into the global task pool,
    this next sibling will be described from a parent_node and the index location in the parent's children list.
    the task which is given to the child will be pointed by filter_idx as index from the CSS Selector Unit list,
    optionally we can enter the previous task accordingly and depth can be set relative to the curr_node depth.
*/
fn push_next_sibling<T: AsyncTraversalTracker<ThreadTask>>(
    task_tracker: &Arc<T>, 
    parent_node: &Arc<Element>, 
    curr_child_idx: usize, 
    curr_depth: usize, 
    filter_idx: usize, 
    add_prev_filter: bool
){

    let next_sibling = parent_node.children.get(curr_child_idx + 1);
    if !next_sibling.is_none() {

        let pushed_node = if let Some(Node::Element(node)) = next_sibling { node.clone() } else { unreachable!() };

        task_tracker.push(ThreadTask{
            curr_node: pushed_node.clone(),
            node_child_pos: curr_child_idx + 1,
            parent_node: parent_node.clone(),
            curr_filter_idx: filter_idx,
            depth: curr_depth
        });

        if add_prev_filter {
            task_tracker.push(ThreadTask{
                curr_node: pushed_node.clone(),
                node_child_pos: curr_child_idx + 1,
                parent_node: parent_node.clone(),
                curr_filter_idx: filter_idx,
                depth: curr_depth
            });
        }
        
    }

}



/*
    Main base for asynchronous traversal in a DOM tree to match a CSS selector, will panic when error are encountered.
    This function can be called by giving the html and css query, number of thread to use, and the data structure to be used as global task pool (must be thread safe).
    
    TODO: return an error message instead of panicking
*/
pub fn async_traversal_base(
    html_text: &str, 
    css_query: &str, 
    core_num: usize, 
    async_tracker: impl AsyncTraversalTracker<ThreadTask> + Send + Sync + 'static
) -> AsyncVec<Arc<Element>> {

    /* Parse the html text */
    let tree: Arc<Element> = parser(html_text).expect("Failed to parse HTML");


    /* Parse and prepare the css selector list */
    let css_filters: Vec<NodeFilter> = CssSelectorParser::new(css_query, false).parse_all();
    let async_filters: Arc<Vec<NodeFilter>>  = Arc::new(css_filters);


    /* Prepare asyncrhonous global task pool, atomic counter for number of active thread, and thread list */
    let atomic_counter: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
    let mut threads: Vec<thread::JoinHandle<()>> = vec![];
    let async_tracker = Arc::new(async_tracker);


    /* Start traversal by pusing the root to the global task pool */
    async_tracker.push(ThreadTask { 
        curr_node: Arc::clone(&tree),
        parent_node: Arc::clone(&tree),
        node_child_pos: 0,
        curr_filter_idx: 0,
        depth: 0,
    });


    /* Prepare the result vector */
    let mut result: Arc<AsyncVec<Arc<Element>>> = Arc::new(AsyncVec::<Arc<Element>>::new());

    for _thread_id in 0..core_num {
        
        /* Prepare each shared data structure for the threads, including the atomic counter*/
        let shared_task_stracker = Arc::clone(&async_tracker);
        let shared_filters = Arc::clone(&async_filters);
        let shared_result: Arc<AsyncVec<Arc<Element>>> = Arc::clone(&result);
        let active_threads_count = Arc::clone(&atomic_counter);

        let thread = thread::spawn(move || {
            
            'thread_loop: loop {

                /* 
                    Currently, this thread is inactive, it will try to get a task from the global task pool (will busy wait until found).
                    A thread is considered done only when global task pool is empty and 
                    number of active thread is zero (i.e. no thread is doing any task)
                */
                let mut task = shared_task_stracker.pop();

                while task.is_none() {
                    if active_threads_count.load(Ordering::SeqCst) == 0 {
                        break 'thread_loop;
                    }
                    task = shared_task_stracker.pop();
                }

                /* When a task is acquired, consider this thread as active */
                active_threads_count.fetch_add(1, Ordering::SeqCst);
                 
                let ThreadTask { 
                    curr_node, 
                    node_child_pos,
                    parent_node, 
                    curr_filter_idx,
                    depth 
                } = task.unwrap();

                
                /* Determine whether the current CSS selector and DOM node described by the task match */
                let curr_filter: &NodeFilter = shared_filters.get(curr_filter_idx).unwrap();
                let node_match_filter = curr_filter.selector.match_node(&curr_node);
                
                if node_match_filter {

                    /* If there are no more selector unit to match from the CSS selector list */
                    if curr_filter_idx + 1 == shared_filters.len() {
                        shared_result.push(curr_node.clone());

                        /* 
                            If the last combinator is a '>' or '~', 
                            there are still possible match somewhere after this 
                        */
                        match curr_filter.prev_combinator {

                            None | Some(Combinator::Descendant) => {
                                push_all_children(
                                    &shared_task_stracker, 
                                    &curr_node, depth, 
                                    curr_filter_idx, 
                                    false);
                            },

                            Some(Combinator::NextSibling) => {
                                push_next_sibling(
                                    &shared_task_stracker, 
                                    &parent_node, 
                                    node_child_pos, 
                                    depth, 
                                    curr_filter_idx, 
                                    false);
                            },

                            _ => {}
                        };

                        active_threads_count.fetch_sub(1, Ordering::SeqCst);
                        continue 'thread_loop;

                    }


                    /* 
                        If there are still more selector unit to match from the CSS selector list, 
                        push new task either from the children or the next sibling accordingly,
                        based on the next combinator.
                    */
                    let next_filter = &shared_filters.get(curr_filter_idx + 1).expect("Invalid css filter index");
                    let next_combinator = next_filter.prev_combinator.as_ref().expect("Unexpected None value Combinator");
                    match next_combinator {

                        Combinator::Child if curr_filter.prev_combinator.as_ref().is_none_or(|val| *val == Combinator::Descendant) => { 
                            push_all_children(
                                &shared_task_stracker, 
                                &curr_node, depth, 
                                curr_filter_idx + 1, 
                                true);
                        },

                        Combinator::Descendant | Combinator::Child => { 
                            push_all_children(
                                &shared_task_stracker, 
                                &curr_node, depth, 
                                curr_filter_idx + 1, 
                                false);
                        }
                        
                        Combinator::DirectNextSibling if curr_filter.prev_combinator == Some(Combinator::NextSibling) => {
                            push_next_sibling(
                                &shared_task_stracker, 
                                &parent_node, 
                                node_child_pos, 
                                depth, 
                                curr_filter_idx + 1, 
                                true);
                        },
                        
                        Combinator::NextSibling | Combinator::DirectNextSibling => {
                            push_next_sibling(
                                &shared_task_stracker, 
                                &parent_node, 
                                node_child_pos, 
                                depth, 
                                curr_filter_idx + 1, 
                                false);
                        }
                    };

                }
                else {
                    
                    /* 
                        If the current DOM Node and CSS selector unit does not match,
                        propagate the current CSS selector unit to the children or next sibling accordingly.
                        We only consider this when the previous combinator is '>' or '~'.
                    */
                    match curr_filter.prev_combinator {
                        
                        Some(Combinator::Descendant) | None => {
                            push_all_children(
                                &shared_task_stracker, 
                                &curr_node, depth, 
                                curr_filter_idx, 
                                false);
                        },


                        Some(Combinator::NextSibling) => {
                            push_next_sibling(
                                &shared_task_stracker, 
                                &parent_node, 
                                node_child_pos, 
                                depth, 
                                curr_filter_idx, 
                                false);
                        },

                        _ => {}
                    }

                }

                
                active_threads_count.fetch_sub(1, Ordering::SeqCst);
            }

        });


        threads.push(thread);
    }


    /* Join all threads which have been spawned */
    for thread in threads {
        thread.join().expect("Failed to join threads");
    };

    /* Return the result vector of DOM Node which matches the CSS Selector Unit list */
    Arc::into_inner(result).expect("Tried to return traversal result before all threads are finished.")

}



/*
    Asynchronous DFS traversal to find matching DOM Node.
    This function is only a wrapper for the main traversal function.
*/
pub fn async_dfs(html_text: &str,  css_query: &str, thread_num: usize) -> Option<Vec<Arc<Element>>> {
    let result: AsyncVec<Arc<Element>> = async_traversal_base(
        html_text, 
        css_query, 
        thread_num, 
        AsyncStack::<ThreadTask>::new()
    );

    result.get_vec()
}



/*
    Asynchronous BFS traversal to find matching DOM Node.
    This function is only a wrapper for the main traversal function.
*/
pub fn async_bfs(html_text: &str,  css_query: &str, thread_num: usize) -> Option<Vec<Arc<Element>>> {
    let result = async_traversal_base(
        html_text, 
        css_query, 
        thread_num, 
        AsyncQueue::<ThreadTask>::new()
    );

    result.get_vec()
}




/* 
    Function to unit test our traversal result, test result are manually checked for now,
    Also see this function for reference on how to use the traversal function (async_dfs and async_bfs).

    CURRENT TEST STATUS: PASSED ALL
*/
#[cfg(test)]
mod tests {

    use super::*; 

    #[test]
    fn test_traversal(){

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
                    <div class="card row-2 col-3 active">Card 5</div>
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
            r##" div "##,
            r##"  body ul > li  a[href ^= "/"][href$="web"  ]  "##,
            r##"html > body main#content-area section.deep-nesting-test div.level-1 div.level-3 > div.level-4 article.deep-article header h3.highlight"##,
            r##"     header ~main~ footer"##,
            r##"     li ~ li > span + div li       "##,
            r##" header.ui-component + main#content-area section.container div.sibling-wrapper h2 ~ p.before + div.target + p.after "##,
            r##" html > body header#main-header.ui-component nav[data-state="active"] ul.nav-list > li.dropdown div.menu-container ul.sub-menu li.featured > a[href="/seo"] "##,
            r##" html > head meta[ charset = "UTF-8"] + title "##,
            r##"   header#main-header nav[data-state="active"] ul.nav-list > li.dropdown span.label   "##,
            r##" section.grid-layout div.card.primary.active[class*="row-2"]   "##,
            r##" main#content-area section.deep-nesting-test div.level-1 div.level-2 div.level-3 > div.level-4 article header h3.highlight "##,
            r##"html[   lang    |="en"] body > main#content-area section[   class ^=  "grid"] div[class ~=  "card"][  class *="col-2"   ] + div  ~ div[class$="active"] "##,
        ];


        let core_num = thread::available_parallelism()
            .expect("Failed to get the number of CPU cores before a pure DFS traversal")
            .get();

        for (idx, selector_query) in testcases.iter().enumerate() {
            eprintln!("\n\n\n##### Traversal Test Case {} #####", idx);

            let dfs_result: Vec<Arc<Element>> = async_dfs(&html, &selector_query, core_num)
                .expect("DFS traversal result is None");

            let bfs_result: Vec<Arc<Element>> = async_dfs(&html, &selector_query, core_num)
                .expect("BFS traversal result is None");

            dbg!(dfs_result);
            dbg!(bfs_result);
        }


    }
}