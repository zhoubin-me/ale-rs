

use ale::{Ale, BundledRom, LoggerMode};
use rand::prelude::*;
use rayon::prelude::*;

fn main() {
    let mut envs = vec![];
    let num_envs = 16;
    Ale::set_logger_mode(LoggerMode::Error);
    for _ in 0..num_envs {
        let mut env = Ale::new(108_000);
        let e = env.load_rom(BundledRom::Breakout);
        match e {
            Ok(_) => (),
            Err(e) => panic!("Failed to load ROM: {:?}", e)
        }
        envs.push(env);
    }

    let n = envs[0].action_dim();
    let start_time = std::time::Instant::now();
    for _ in 0..10000 {
        envs.par_iter_mut().for_each(|e| {
            let action = thread_rng().gen_range(0..n);
            e.act(action);
            if e.is_game_over() {
                e.reset_game();
            }
        });
    }
    let elapsed = start_time.elapsed();
    println!("Time elapsed: {:?}", elapsed);

}

