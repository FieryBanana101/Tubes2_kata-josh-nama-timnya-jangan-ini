use std::sync::{Arc};
use std::marker::{Send, Sync};
use std::thread;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::tokenizer::{parser, Node, Element};
use crate::css_selector::*;
use crate::async_util::*;



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




pub fn async_traversal_base(
    html_text: &str, 
    css_query: &str, 
    core_num: usize, 
    async_tracker: impl AsyncTraversalTracker<ThreadTask> + Send + Sync + 'static
) -> AsyncVec<Arc<Element>> {

    let tree: Arc<Element> = parser(html_text).expect("Failed to parse HTML");

    let css_filters: Vec<NodeFilter> = CssSelectorParser::new(css_query, false).parse_all();
    let async_filters: Arc<Vec<NodeFilter>>  = Arc::new(css_filters);

    let atomic_counter: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
    let mut threads: Vec<thread::JoinHandle<()>> = vec![];

    let async_tracker = Arc::new(async_tracker);

    async_tracker.push(ThreadTask { 
        curr_node: Arc::clone(&tree),
        parent_node: Arc::clone(&tree),
        node_child_pos: 0,
        curr_filter_idx: 0,
        depth: 0,
    });

    
    let mut result: Arc<AsyncVec<Arc<Element>>> = Arc::new(AsyncVec::<Arc<Element>>::new());

    for _thread_id in 0..core_num {
        
        let shared_task_stracker = Arc::clone(&async_tracker);
        let shared_filters = Arc::clone(&async_filters);
        let shared_result: Arc<AsyncVec<Arc<Element>>> = Arc::clone(&result);

        let active_threads_count = Arc::clone(&atomic_counter);


        let thread = thread::spawn(move || {
            
            'thread_loop: loop {
                let mut task = shared_task_stracker.pop();

                while task.is_none() {
                    if active_threads_count.load(Ordering::SeqCst) == 0 {
                        break 'thread_loop;
                    }
                    task = shared_task_stracker.pop();
                }

                active_threads_count.fetch_add(1, Ordering::SeqCst);
                
                let ThreadTask { 
                    curr_node, 
                    node_child_pos,
                    parent_node, 
                    curr_filter_idx,
                    depth 
                } = task.unwrap();

                
                let curr_filter: &NodeFilter = shared_filters.get(curr_filter_idx).unwrap();
                let node_match_filter = curr_filter.selector.match_node(&curr_node);
                
                if node_match_filter {

                    if curr_filter_idx + 1 == shared_filters.len() {
                        shared_result.push(curr_node.clone());

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
                else{
                    
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


    for thread in threads {
        thread.join().expect("Failed to join threads");
    };

    Arc::into_inner(result).expect("Tried to return traversal result before all threads are finished.")

}



pub fn async_dfs(html_text: &str,  css_query: &str, thread_num: usize) -> Option<Vec<Arc<Element>>> {
    let result: AsyncVec<Arc<Element>> = async_traversal_base(
        html_text, 
        css_query, 
        thread_num, 
        AsyncStack::<ThreadTask>::new()
    );

    result.get_vec()
}



pub fn async_bfs(html_text: &str,  css_query: &str, thread_num: usize) -> Option<Vec<Arc<Element>>> {
    let result = async_traversal_base(
        html_text, 
        css_query, 
        thread_num, 
        AsyncQueue::<ThreadTask>::new()
    );

    result.get_vec()
}




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
            r##"  body ul > li  a[href ^= "/"][href$="web"   ]  "##,
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