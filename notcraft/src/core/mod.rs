pub enum StateTransition {
    Push(Box<dyn State>),
    Swap(Box<dyn State>),
    Pop,
    Quit,
    None,
}

pub trait State {
    fn update(&mut self) -> StateTransition;

    fn input(&mut self);
}

pub struct Game {
    world: World,
    sates: Vec<Box<dyn State>>,
}

impl Game {
    pub fn run() {
        
    }
}
