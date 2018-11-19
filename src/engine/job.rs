use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::mpsc;
use std::thread;

pub trait Worker: Send + 'static {
    type Input: Send + 'static;
    type Output: Send + 'static;

    fn compute(&mut self, input: &Self::Input) -> Self::Output;
}

pub struct WorkerHandle<W: Worker> {
    working: bool,
    sender: mpsc::Sender<W::Input>,
    handle: thread::JoinHandle<()>,
}

impl<W: Worker> WorkerHandle<W> {
    pub fn new(
        name: String,
        id: usize,
        worker_tx: mpsc::Sender<(usize, W::Input, W::Output)>,
        mut worker: W,
    ) -> WorkerHandle<W> {
        let (sender, rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name(name)
            .spawn(move || {
                while let Ok(request) = rx.recv() {
                    let res = worker.compute(&request);
                    match worker_tx.send((id, request, res)) {
                        Err(_) => break,
                        _ => (),
                    }
                }
            })
            .unwrap();

        WorkerHandle {
            working: false,
            handle: thread,
            sender,
        }
    }

    fn submit(&mut self, value: W::Input) {
        self.working = true;
        self.sender
            .send(value)
            .expect("worker thread unexpectedly shut down")
    }
}

pub struct Service<W: Worker> {
    workers: Vec<WorkerHandle<W>>,
    worker_tx: mpsc::Sender<(usize, W::Input, W::Output)>,
    service_rx: mpsc::Receiver<(usize, W::Input, W::Output)>,
    work_queue: VecDeque<W::Input>,
    finished_queue: Vec<(W::Input, W::Output)>,
}

impl<W: Worker> Service<W> {
    pub fn from_iter<I>(name: &str, workers: I) -> Self
    where
        I: IntoIterator<Item = W>,
    {
        let (worker_tx, service_rx) = mpsc::channel();

        let workers = workers
            .into_iter()
            .enumerate()
            .map(|(thread_num, worker)| {
                let thread_name = format!("{} (Worker #{})", name, thread_num);
                let worker_tx = worker_tx.clone();
                WorkerHandle::new(thread_name, thread_num, worker_tx, worker)
            })
            .collect::<Vec<_>>();

        Service {
            workers,
            worker_tx,
            service_rx,
            work_queue: VecDeque::new(),
            finished_queue: Vec::new(),
        }
    }

    pub fn new(name: &str, num_workers: usize, worker: W) -> Self
    where
        W: Clone,
    {
        use std::iter;
        Self::from_iter(name, iter::repeat(worker).take(num_workers))
    }

    pub fn dispatch(&mut self, request: W::Input) {
        if let Some(worker) = self.workers.iter_mut().find(|item| !item.working) {
            // TODO: remove shut-down thread from pool
            worker.submit(request)
        } else {
            self.work_queue.push_back(request);
        }
    }

    pub fn update(&mut self) {
        for (id, tx, rx) in self.service_rx.try_iter() {
            self.workers[id].working = false;
            self.finished_queue.push((tx, rx));
        }

        for worker in self
            .workers
            .iter_mut()
            .filter(|worker| !worker.working)
            .take(self.work_queue.len())
        {
            // we can unwrap here because we will never iterate more times than there are items in the list
            worker.submit(self.work_queue.pop_front().unwrap());
        }
    }

    pub fn poll(&mut self) -> impl Iterator<Item = (W::Input, W::Output)> + '_ {
        self.finished_queue.drain(..)
    }
}
