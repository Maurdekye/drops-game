use std::{
    error::Error,
    fs::File,
    io::{stdout, Write},
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering::SeqCst},
};

use clap::Parser;
use rand::prelude::*;
use rayon::prelude::*;

fn simulate_strategy(args: &Args, strategy: impl Fn(usize) -> bool) -> usize {
    let mut rng = thread_rng();

    let mut counter = 0;
    let mut drops = 0;

    for _ in 0..args.sim_steps_per_strategy {
        let counter_active = strategy(counter);

        // kill enemy
        counter += 1;

        if counter >= args.max_counter_value {
            // guaranteed pity drop given
            counter = 0;
            drops += 1;
            continue;
        }

        // check for random drop from killed enemy
        let drop_chance = if counter_active {
            args.base_drop_rate + args.counter_multiplier * (counter as f64)
        } else {
            args.base_drop_rate
        };

        let drop_roll: f64 = rng.gen();
        if drop_roll < drop_chance {
            // random item drop acquired
            drops += 1;
            if counter_active {
                counter = 0;
            }
        }
    }

    drops
}

#[derive(Parser)]
struct Args {
    #[clap(short, long, default_value_t = 0.001)]
    base_drop_rate: f64,

    #[clap(short, long, default_value_t = 0.000002)]
    counter_multiplier: f64,

    #[clap(short, long, default_value_t = 1000)]
    max_counter_value: usize,

    #[clap(short, long, default_value_t = 10_000_000_000)]
    sim_steps_per_strategy: usize,

    #[clap(short, long)]
    out: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let strategy_finish_counter = AtomicUsize::new(0);
    let num_strategies = args.max_counter_value + 1;

    let results: Vec<_> = (0..num_strategies)
        .into_par_iter()
        .map(|strategy| {
            let drops = simulate_strategy(&args, |counter| counter < strategy);
            let strategy_finish_counter_value = strategy_finish_counter.fetch_add(1, SeqCst);
            print!(
                "\rfinished {}/{}",
                strategy_finish_counter_value + 1,
                num_strategies
            );
            stdout().flush().unwrap();

            drops
        })
        .collect();
    println!();

    let mut max_drops_strategy = 0;
    let mut max_drops = 0;
    println!("Simulation results:");
    for (i, &drops) in results.iter().enumerate() {
        if drops > max_drops {
            max_drops = drops;
            max_drops_strategy = i;
        }
        println!(
            "strategy {i}: {drops} drops, {} drops per kill",
            (drops as f64) / (args.sim_steps_per_strategy as f64)
        );
    }
    println!("Strategy with the most drops was {max_drops_strategy}, with {max_drops} drops");

    if let Some(out) = args.out {
        let mut out_file = File::create(out)?;
        for (i, drops) in results.into_iter().enumerate() {
            writeln!(
                out_file,
                "{i},{drops},{}",
                (drops as f64) / (args.sim_steps_per_strategy as f64)
            )?;
        }
    }

    Ok(())
}
