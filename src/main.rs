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

trait Strategy {
    fn decide(&self, counter: usize) -> bool;

    fn show(&self, max: usize) {
        for i in 0..=max {
            println!("{i}: {}", self.decide(i));
        }
    }
}

fn simulate_strategy(args: &Args, strategy: &impl Strategy) -> usize {
    let mut rng = thread_rng();

    let mut counter = 0;
    let mut drops = 0;

    for _ in 0..args.sim_steps_per_strategy {
        let counter_active = strategy.decide(counter);

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

struct NaiveThreshold(usize);
impl Strategy for NaiveThreshold {
    fn decide(&self, counter: usize) -> bool {
        counter > self.0
    }
}

struct NaiveInverseThreshold(usize);
impl Strategy for NaiveInverseThreshold {
    fn decide(&self, counter: usize) -> bool {
        counter < self.0
    }
}

#[derive(Clone)]
struct XorInverseThresholds(Vec<usize>);
impl Strategy for XorInverseThresholds {
    fn decide(&self, counter: usize) -> bool {
        self.0.iter().filter(|&&t| counter > t).count() % 2 == 0
    }
}

#[derive(Clone)]
struct XorThresholds(Vec<usize>);
impl Strategy for XorThresholds {
    fn decide(&self, counter: usize) -> bool {
        self.0.iter().filter(|&&t| counter < t).count() % 2 == 0
    }
}

#[derive(Parser)]
struct Args {
    #[clap(short, long, default_value_t = 0.001)]
    base_drop_rate: f64,

    #[clap(short, long, default_value_t = 0.000002)]
    counter_multiplier: f64,

    #[clap(short, long, default_value_t = 1000)]
    max_counter_value: usize,

    #[clap(short, long, default_value_t = 1_000_000_000)]
    sim_steps_per_strategy: usize,

    #[clap(short, long)]
    out: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut thresholds = vec![];

    let results = loop {
        println!(
            "Determining optimal placement of threshold {}",
            thresholds.len() + 1
        );
        let strategy_finish_counter = AtomicUsize::new(0);
        let num_strategies = args.max_counter_value + 1;

        let results: Vec<_> = (0..num_strategies)
            .into_par_iter()
            .map(|strategy_index| {
                let mut my_thresholds = thresholds.clone();
                my_thresholds.push(strategy_index);
                let drops = simulate_strategy(&args, &XorInverseThresholds(my_thresholds));
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

        let (max_strategy, &max_drops) = results
            .iter()
            .enumerate()
            .max_by_key(|(_, &d)| d)
            .expect("Results vector will be non-empty");

        if max_strategy == num_strategies - 1 {
            println!("No further strategy optimization can be made");
            break results;
        }

        println!("Determined optimal strategy index to be #{max_strategy}, yielding {max_drops} drops at a rate of {} drops per kill", (max_drops as f64) / (args.sim_steps_per_strategy as f64));

        if thresholds.contains(&max_strategy) {
            thresholds.retain(|&x| x != max_strategy);
        } else {
            thresholds.push(max_strategy);
        }

        println!("New thresholds are {thresholds:?}");
    };

    println!("Final simulation results:");
    for (i, &drops) in results.iter().enumerate() {
        println!(
            "strategy {i}: {drops} drops, {} drops per kill",
            (drops as f64) / (args.sim_steps_per_strategy as f64)
        );
    }

    println!("Final strategy thresholds: {thresholds:?}");

    if let Some(out) = args.out {
        let filepath_string = out.to_string_lossy().to_string();
        let mut out_file = File::create(out)?;
        for (i, drops) in results.into_iter().enumerate() {
            writeln!(
                out_file,
                "{i},{drops},{}",
                (drops as f64) / (args.sim_steps_per_strategy as f64)
            )?;
        }
        println!("Saved results to {filepath_string}");
    }

    Ok(())
}

#[test]
fn test1() {
    let strategy = XorInverseThresholds(vec![541, 950]);
    strategy.show(1000);
}

#[test]
fn plot() -> Result<(), Box<dyn Error>> {
    use csv::Reader;
    use plotters::prelude::*;
    use plotters::style::full_palette::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Record(usize, usize, f64);

    for data_source in ["data", "early-data", "xor-data", "xor-data-3"] {
        let in_file = format!("{data_source}.csv");
        let out_img = format!("{data_source}.png");
        let root = BitMapBackend::new(&out_img, (1280, 720)).into_drawing_area();
        let points: Vec<Record> = Reader::from_path(in_file)?
            .deserialize()
            .collect::<Result<_, _>>()?;
        let mut plot =
            ChartBuilder::on(&root).build_cartesian_2d(0usize..1000, 0.00185f64..0.0021)?;
        plot.draw_series(
            points
                .iter()
                .map(|Record(index, _, rate)| Circle::new((*index, *rate), 2, GREEN_500.filled())),
        )?;
        root.present()?;
        println!("saved {out_img}");
    }

    Ok(())
}
