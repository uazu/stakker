use crate::task::*;
use crate::*;
use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Instant;

// This tests the Task mechanism, but without needing async/await
test_fn!(
    fn task_test() {
        let now = Instant::now();
        let mut stakker = Stakker::new(now);
        let s = &mut stakker;

        let progress0 = Rc::new(RefCell::new(Vec::new()));

        struct MyTask {
            deferrer: Deferrer,
            count: u32,
            progress: Rc<RefCell<Vec<u32>>>,
        }
        impl TaskTrait for MyTask {
            fn resume(mut self: Pin<&mut Self>, core: &mut Core) {
                self.progress.borrow_mut().push(self.count);
                self.count += 1;
                if self.count < 4 {
                    let mut task = Task::from_context(&self.deferrer).unwrap();
                    call!([core], |s| task.resume(s));
                }
            }
        }

        let my_task = MyTask {
            deferrer: s.deferrer(),
            count: 0,
            progress: progress0.clone(),
        };
        let mut task = Task::new(s, my_task);
        task.resume(s);

        // Task reference should now be held in item on queue
        drop(task);

        // Executing the queue causes the task to be repeatedly resumed
        // and queued until it is done
        s.run(now, false);

        assert_eq!(vec![0, 1, 2, 3], *progress0.borrow());
    }
);
