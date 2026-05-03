use std::marker::{Send, Sync};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use crate::async_util::*;
use crate::css_selector::*;
use crate::html::{parser, Element, Node};

/*
    Push all children of the current node into the global task pool,
    the task which is given to the children will be pointed by filter_idx as index from the CSS Selector Unit list,
    optionally we can enter the previous task accordingly and depth can be set relative to the curr_node depth.
*/
fn push_all_children<T: AsyncTraversalTracker<ThreadTask>>(
    task_tracker: &Arc<T>,
    curr_node: &Arc<Element>,
    curr_depth: usize,
    selector_list_idx: usize,
    selector_unit_idx: usize,
    add_prev_filter: bool,
    reversed: bool
) {
    let iter  = (&curr_node).children.iter().enumerate();
    let rev_iter = (&curr_node).children.iter().enumerate().rev();
    let children: Vec<(usize, &Node)> = if reversed { rev_iter.collect() } else { iter.collect() };
    
    for (idx, node) in children {
        if let Node::Element(child) = node {
            task_tracker.push(ThreadTask {
                curr_node: child.clone(),
                node_child_pos: idx,
                parent_node: curr_node.clone(),
                selector_list_idx: selector_list_idx,
                selector_unit_idx: selector_unit_idx,
                depth: curr_depth + 1,
            });

            if add_prev_filter {
                task_tracker.push(ThreadTask {
                    curr_node: child.clone(),
                    node_child_pos: idx,
                    parent_node: curr_node.clone(),
                    selector_list_idx: selector_list_idx,
                    selector_unit_idx: selector_unit_idx - 1,
                    depth: curr_depth + 1,
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
    selector_list_idx: usize,
    selector_unit_idx: usize,
    add_prev_filter: bool,
) {
    let next_sibling = parent_node.children.get(curr_child_idx + 1);
    if !next_sibling.is_none() {
        let pushed_node = if let Some(Node::Element(node)) = next_sibling {
            node.clone()
        } else {
            unreachable!()
        };

        task_tracker.push(ThreadTask {
            curr_node: pushed_node.clone(),
            node_child_pos: curr_child_idx + 1,
            parent_node: parent_node.clone(),
            selector_list_idx: selector_list_idx,
            selector_unit_idx: selector_unit_idx,
            depth: curr_depth,
        });

        if add_prev_filter {
            task_tracker.push(ThreadTask {
                curr_node: pushed_node.clone(),
                node_child_pos: curr_child_idx + 1,
                parent_node: parent_node.clone(),
                selector_list_idx: selector_list_idx,
                selector_unit_idx: selector_unit_idx,
                depth: curr_depth,
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
    tree: Arc<Element>,
    css_query: &str,
    core_num: usize,
    async_tracker: impl AsyncTraversalTracker<ThreadTask> + Send + Sync + 'static,
    reversed: bool
) -> (AsyncVec<Arc<Element>>, AsyncVec<Vec<usize>>) {

    /* Parse and prepare the css selector list */
    let mut css_filters: Vec<Vec<NodeFilter>> = Vec::new();
    let complex_selector_list: Vec<&str> = css_query.split(',').collect();
    for complex_selector in complex_selector_list {
        let css_filter: Vec<NodeFilter> =
            CssSelectorParser::new(complex_selector, false).parse_all();
        css_filters.push(css_filter);
    }

    let async_filters: Arc<Vec<Vec<NodeFilter>>> = Arc::new(css_filters);

    /* Prepare asyncrhonous global task pool, atomic counter for number of active thread, and thread list */
    let atomic_counter: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
    let mut threads: Vec<thread::JoinHandle<()>> = vec![];
    let async_tracker = Arc::new(async_tracker);

    /* Start traversal by pushing the root for each css selector unit to the global task pool */
    for (idx, _) in async_filters.iter().enumerate() {
        async_tracker.push(ThreadTask {
            curr_node: Arc::clone(&tree),
            node_child_pos: 0,
            parent_node: Arc::clone(&tree),
            selector_list_idx: idx,
            selector_unit_idx: 0,
            depth: 0,
        });
    }

    /* Prepare the result vector and global node id tracker to avoid duplication in result */
    let result: Arc<AsyncVec<Arc<Element>>> = Arc::new(AsyncVec::<Arc<Element>>::new());
    let id_tracker: Arc<AsyncHashSet<usize>> = Arc::new(AsyncHashSet::<usize>::new());
    let path_tracker_list: Arc<AsyncVec<Vec<usize>>> = Arc::new(AsyncVec::<Vec<usize>>::new());

    for _thread_id in 0..core_num {
        /* Prepare each shared data structure for the threads, including the atomic counter*/
        let shared_task_stracker = Arc::clone(&async_tracker);
        let shared_filters = Arc::clone(&async_filters);
        let shared_result = Arc::clone(&result);
        let shared_dup_tracker = Arc::clone(&id_tracker);
        let shared_path_tracker_list = Arc::clone(&path_tracker_list);
        let active_threads_count = Arc::clone(&atomic_counter);

        let thread = thread::spawn(move || {
            let mut path_tracker: Vec<usize>= Vec::new();
            'thread_loop: loop {
                /*
                    Currently, this thread is inactive, it will try to get a task from the global task pool (will busy wait until found).
                    A thread is considered done only when global task pool is empty and
                    number of active thread is zero (i.e. no thread is doing any task)
                */
                let mut task = shared_task_stracker.pop();

                while task.is_none() {
                    if active_threads_count.load(Ordering::SeqCst) == 0 {
                        shared_path_tracker_list.push(path_tracker);
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
                    selector_list_idx,
                    selector_unit_idx,
                    depth,
                } = task.unwrap();

                path_tracker.push(curr_node.global_id);

                /* Determine whether the current CSS selector and DOM node described by the task match */
                let filter_list = shared_filters.get(selector_list_idx).unwrap();
                let filter = filter_list.get(selector_unit_idx).unwrap();

                let node_match_filter =
                    filter
                        .selector
                        .match_node(&curr_node, node_child_pos, &parent_node);

                if node_match_filter {
                    /* If there are no more selector unit to match from the CSS selector list */
                    if selector_unit_idx + 1 == filter_list.len() {
                        /* The current node match the css selector but check the ID tracker to avoid duplicate result */
                        let curr_node_raw_ptr = Arc::into_raw(curr_node.clone()) as usize;
                        if shared_dup_tracker.insert(curr_node_raw_ptr) {
                            shared_result.push(curr_node.clone());
                        }

                        /*
                            If the last combinator is a '>' or '~',
                            there are still possible match somewhere after this
                        */
                        match filter.prev_combinator {
                            None | Some(Combinator::Descendant) => {
                                push_all_children(
                                    &shared_task_stracker,
                                    &curr_node,
                                    depth,
                                    selector_list_idx,
                                    selector_unit_idx,
                                    false,
                                    reversed
                                );
                            }

                            Some(Combinator::NextSibling) => {
                                push_next_sibling(
                                    &shared_task_stracker,
                                    &parent_node,
                                    node_child_pos,
                                    depth,
                                    selector_list_idx,
                                    selector_unit_idx,
                                    false,
                                );
                            }

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
                    let next_filter = filter_list
                        .get(selector_unit_idx + 1)
                        .expect("Inavlid css selector unit index for a filter list");

                    let next_combinator = next_filter
                        .prev_combinator
                        .as_ref()
                        .expect("Unexpected None value Combinator");

                    match next_combinator {
                        Combinator::Child
                            if filter
                                .prev_combinator
                                .as_ref()
                                .is_none_or(|val| *val == Combinator::Descendant) =>
                        {
                            push_all_children(
                                &shared_task_stracker,
                                &curr_node,
                                depth,
                                selector_list_idx,
                                selector_unit_idx + 1,
                                true,
                                reversed
                            );
                        }

                        Combinator::Descendant | Combinator::Child => {
                            push_all_children(
                                &shared_task_stracker,
                                &curr_node,
                                depth,
                                selector_list_idx,
                                selector_unit_idx + 1,
                                false,
                                reversed
                            );
                        }

                        Combinator::DirectNextSibling
                            if filter.prev_combinator == Some(Combinator::NextSibling) =>
                        {
                            push_next_sibling(
                                &shared_task_stracker,
                                &parent_node,
                                node_child_pos,
                                depth,
                                selector_list_idx,
                                selector_unit_idx + 1,
                                true,
                            );
                        }

                        Combinator::NextSibling | Combinator::DirectNextSibling => {
                            push_next_sibling(
                                &shared_task_stracker,
                                &parent_node,
                                node_child_pos,
                                depth,
                                selector_list_idx,
                                selector_unit_idx + 1,
                                false,
                            );
                        }
                    };
                } else {
                    /*
                        If the current DOM Node and CSS selector unit does not match,
                        propagate the current CSS selector unit to the children or next sibling accordingly.
                        We only consider this when the previous combinator is '>' or '~'.
                    */
                    match filter.prev_combinator {
                        Some(Combinator::Descendant) | None => {
                            push_all_children(
                                &shared_task_stracker,
                                &curr_node,
                                depth,
                                selector_list_idx,
                                selector_unit_idx,
                                false,
                                reversed
                            );
                        }

                        Some(Combinator::NextSibling) => {
                            push_next_sibling(
                                &shared_task_stracker,
                                &parent_node,
                                node_child_pos,
                                depth,
                                selector_list_idx,
                                selector_unit_idx,
                                false,
                            );
                        }

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
    }

    /* Return the result vector of DOM Node which matches the CSS Selector Unit list and path of each thread */
    (Arc::into_inner(result)
        .expect("Tried to return traversal result before all threads are finished."),
    
    Arc::into_inner(path_tracker_list)
        .expect("Tried to return path tracker result before all threads are finished."))

}

/*
    Asynchronous DFS traversal to find matching DOM Node.
    This function is only a wrapper for the main traversal function.
*/
pub fn async_dfs(root: Arc<Element>, css_query: &str, thread_num: usize) -> (Option<Vec<Arc<Element>>>, Option<Vec<Vec<usize>>>) {
    let (result, path_tracker) = async_traversal_base(
        root,
        css_query,
        thread_num,
        AsyncStack::<ThreadTask>::new(),
        true
    );

    (result.get_vec(), path_tracker.get_vec())
}

/*
    Asynchronous BFS traversal to find matching DOM Node.
    This function is only a wrapper for the main traversal function.
*/
pub fn async_bfs(root: Arc<Element>, css_query: &str, thread_num: usize) -> (Option<Vec<Arc<Element>>>, Option<Vec<Vec<usize>>>) {
    let (result, path_tracker) = async_traversal_base(
        root,
        css_query,
        thread_num,
        AsyncQueue::<ThreadTask>::new(),
        false
    );

    (result.get_vec(), path_tracker.get_vec())
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
    fn test_traversal() {
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

            <div id="test-root">
                <div id="not-empty-whitespace"> </div>
                <div id="really-empty"></div>

                <nav>
                    <a href="https://google.com" id="link-1">Link</a>
                    <a id="not-a-link">Anchor without href</a>
                </nav>

                <form>
                    <input type="text" required id="req-input">
                    <input type="checkbox" id="opt-input">
                    <textarea readonly id="readonly-text">Can't touch this</textarea>
                    <div contenteditable id="editable-div">
                        <p>I am editable</p>
                        <span contenteditable="false" id="locked-span">I am nested and locked</span>
                    </div>
                </form>

                <section id="gauntlet">
                    <p id="p1">First P</p>
                    Text Node (Ignored by -of-type)
                    <div id="d1">First Div</div>
                    <p id="p2">Second P</p>
                    <span id="s1">Only Span</span>
                    <div id="d2">Last Div</div>
                    <p id="p3">Last P</p>
                </section>

                <div id="outer-only">
                    <div id="inner-only">
                        <p id="lone-p">Lone P</p>
                    </div>
                </div>
            </div>

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
            r##"p, div header, a"##,
            r##":empty"##,
            r##":any-link"##,
            r##":required"##,
            r##":optional"##,
            r##":read-write"##,
            r##":read-only"##,
            r##"p:first-of-type"##,
            r##"p:last-of-type"##,
            r##"section > :last-child"##,
            r##"span:only-of-type"##,
            r##"#inner-only:only-child"##,
            r##"div > p:only-child"##,
            r##"#not-empty-whitespace:empty"##,
            r##"#editable-div p:read-write"##,
            r##"#locked-span:read-only"##,
            r##"input:required, textarea:read-only"##,
            r##"div:first-child, div:last-child"##,
        ];

        let core_num = thread::available_parallelism()
            .expect("Failed to get the number of CPU cores before a pure DFS traversal")
            .get();

        let (tree, _) = parser(&html).unwrap();

        for (idx, selector_query) in testcases.iter().enumerate() {
            eprintln!("\n\n\n##### Traversal Test Case {} #####", idx);

            let (dfs_result, dfs_path) =
                async_dfs(tree.clone(), &selector_query, core_num);

            let (bfs_result, bfs_path) =
                async_bfs(tree.clone(), &selector_query, core_num);

            dbg!(selector_query);
            dbg!(dfs_result);
            dbg!(dfs_path);
            eprintln!("\n");
            //dbg!(bfs_result);
        }
    }
}
