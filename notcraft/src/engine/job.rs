// use crossbeam::deque::{self, Steal};
// use std::{
//     sync::{mpsc, Mutex},
//     thread,
// };

// pub trait Worker: Send + 'static {
//     type Input: Send + 'static;
//     type Output: Send + 'static;

//     fn compute(&mut self, input: &Self::Input) -> Self::Output;
// }

// fn spawn_worker<W: Worker>(
//     name: String,
//     inputs: &deque::Stealer<W::Input>,
//     tx: &mpsc::Sender<(W::Input, W::Output)>,
//     mut worker: W,
// ) -> thread::JoinHandle<()> {
//     let inputs = inputs.clone();
//     let tx = tx.clone();
//     thread::Builder::new()
//         .name(name)
//         .spawn(move || loop {
//             match inputs.steal() {
//                 // either the queue is empty and we want to wait for more
// work to do, so we retry,                 // or we are forced to retry
//                 Steal::Empty | Steal::Retry => thread::yield_now(),

//                 // We got the data! now process the request and send it
//                 Steal::Success(request) => {
//                     let res = worker.compute(&request);
//                     match tx.send((request, res)) {
//                         // We get an error if the recv side has shut down,
// and it will only shut                         // down when we're done with
// the sericde anyways, so if we get an error, we                         //
// exit the loop/thread                         Err(_) => break,
//                         _ => (),
//                     }
//                 }
//             }
//         })
//         .unwrap()
// }

// #[derive(Debug)]
// pub struct Service<W: Worker> {
//     requester: Mutex<deque::Worker<W::Input>>,
//     receiver: Mutex<mpsc::Receiver<(W::Input, W::Output)>>,
// }

// impl<W: Worker> Service<W> {
//     pub fn from_iter<I>(name: &str, workers: I) -> Self
//     where
//         I: IntoIterator<Item = W>,
//     {
//         let request_inserter = deque::Worker::new_fifo();
//         let request_stealer = request_inserter.stealer();
//         let (response_tx, response_rx) = mpsc::channel();

//         for (num, worker) in workers.into_iter().enumerate() {
//             let thread_name = format!("{} (Worker #{})", name, num);
//             spawn_worker(thread_name, &request_stealer, &response_tx,
// worker);         }

//         Service {
//             requester: Mutex::new(request_inserter),
//             receiver: Mutex::new(response_rx),
//         }
//     }

//     pub fn new(name: &str, num_workers: usize, worker: W) -> Self
//     where
//         W: Clone,
//     {
//         use std::iter;
//         Self::from_iter(name, iter::repeat(worker).take(num_workers))
//     }

//     pub fn request(&mut self, request: W::Input) {
//         self.requester.lock().unwrap().push(request);
//     }

//     // pub fn cancel(&mut self, request: &W::Input) {}

//     pub fn gather(&mut self) -> impl Iterator<Item = (W::Input, W::Output)> +
// '_ {         self.receiver.lock().unwrap().try_iter()
//     }
// }
