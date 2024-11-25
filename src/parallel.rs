use crate::{Ale, BundledRom, LoggerMode};
use rayon::prelude::*;

pub struct ParallelAtari {
    envs: Vec<Ale>,
    transitions: Vec<(Vec<u8>, u8, i32, bool, bool, Vec<u8>)>
}

impl ParallelAtari {
    fn new(num_envs: u32, max_frames: u32) -> Self {
        let mut envs = vec![];
        Ale::set_logger_mode(LoggerMode::Error);
        for _ in 0..num_envs {
            let mut env = Ale::new(max_frames);
            env.load_rom(BundledRom::Breakout);
            envs.push(env);
        }
        let transitions = vec![];
        ParallelAtari {envs, transitions}
    }

    fn reset(&mut self) -> Vec<Vec<u8>> {
        self.envs.par_iter_mut().map(|e| {
            e.reset_game();
            e.screen()
        }).collect()
    }

    fn step(&mut self, actions: Vec<u8>){
        let transitions = self.envs.par_iter_mut().zip(actions).map(|(e, a)| {
            let cur_obs = e.screen();
            let (r, live_loss, truncation) = e.act(a);
            let terminal = e.is_game_over();
            if terminal || truncation {
                e.reset_game();
            }
            let next_obs = e.screen();
            let done = terminal || live_loss;
            (cur_obs, a, r, done, truncation, next_obs)
        }).collect::<Vec<(Vec<u8>, u8, i32, bool, bool, Vec<u8>)>>();
        self.transitions.extend(transitions);
    }
}



