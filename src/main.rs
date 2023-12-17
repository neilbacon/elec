use anyhow::{Error, Result, anyhow, Context};
use chrono::{NaiveDate};
use chrono::prelude::*;
use clap::Parser;
use csv::ReaderBuilder;
use sscanf::sscanf;
use std::path::Path;
use log::{debug, info};
use env_logger;
use std::collections::HashSet;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]

struct Args {
    /// Consumption Tariff CSV file
    #[arg(short='t', long)]
    consumption_tariff: String,

    /// Consumption Data CSV file
    #[arg(short, long)]
    consumption: String,

    /// Feedin Tariff CSV file
    #[arg(short='u', long)]
    feedin_tariff: Option<String>,

    /// Feedin Data CSV file
    #[arg(short, long)]
    feedin: Option<String>,

    /// Daily supply charge
    #[arg(short, long)]
    daily: String,

    /// Public Holidays
    #[arg(short, long)]
    public_holidays: Option<String>,
}

fn minutes_since_midnight(hhmmss: &str) -> Result<i32> {
    sscanf!(hhmmss, "{i32}:{i32}:{i32}")
    .map(|(hh, mm, _ss)| mm + 60 * hh)
    .or_else(|e| Err(anyhow!("minutes_since_midnight: error {}", e))) // convert sscanf::Error to anyhow::Error
}

#[derive(Debug)]
struct Tariff {
    day_start: i16,  // Day Start (0 for Monday), todo: later try u16 to see if its painful
    day_end: i16,    // Day End (Exclusive)
    time_start: i32, // Time Start (min since midnight)
    time_end: i32,   // Time End (Exclusive)
    tariff: f64,     // $/kWh
    _name: String,    // Tariff Name
}

fn load_tariff(csv_tariff: &String) -> Result<Vec<Tariff>> {
    info!("load_tariff: loading CSV file {}", csv_tariff);
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(Path::new(csv_tariff))?;
    
    reader.records()
    .map(|record| -> Result<Tariff> {
        let r = record?;
        debug!("load_tariff: record: {:?}", r);
        Ok(Tariff {
            day_start:  r[0].parse::<i16>()?, 
            day_end:    r[1].parse::<i16>()?,  
            time_start: minutes_since_midnight(&r[2])?,  
            time_end:   minutes_since_midnight(&r[3])?,  
            tariff:     r[4].parse::<f64>()?,  
            _name:      r[5].to_string(),
        })
    })
    .collect() // 1st error, or the vector
}

fn load_supply_charge(csv_tariff: &String) -> Result<f64> {
    info!("load_supply_charge: loading CSV file {}", csv_tariff);
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(Path::new(csv_tariff))?;

    let r = reader.records().next().context("'{}' missing data line 1")??;
    debug!("load_supply_charge: record: {:?}", r);
    Ok(r[0].parse::<f64>()?)
}

fn load_public_holidays(csv: &str) -> Result<HashSet<String>> {
    info!("load_public_holidays: loading CSV file {}", csv);
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(Path::new(csv))?;

    reader.records()
    .map(|record| -> Result<String> {
        let r = record?;
        debug!("load_public_holidays: record: {:?}", r);
        Ok(r[0].trim().to_string())
    })
    .collect()
} 

// Lookup $/kWh for the day of the week (0 for Monday) and time of day
// For time of the day, we only check that the start of the consumption interval is within the tariff time interval,
// assuming that consumption intervals always fall within single tariff intervals.
fn lookup_tariff(dow: i16, min_since_midnight: i32, tariff: &Vec<Tariff>) -> Result<f64> {
    tariff.iter().find(|x| 
        x.day_start <= dow &&
        x.day_end > dow &&
        x.time_start <= min_since_midnight &&
        x.time_end > min_since_midnight
    )
    .map(|t| t.tariff)
    .context(format!("lookup_tariff: no tarriff for day of week {} and min_since_midnight {}", dow, min_since_midnight))
}

// Apply tariff to energy (either consumption or feedin), returning (line_count, col_count, price)
fn price_energy<F>(csv_energy: &String, tariff: F, holidays: &HashSet<String>) -> Result<(usize, usize, f64)> where
F: Fn(i16, i32) -> Result<f64> {
    info!("price_energy: loading CSV file {}", csv_energy);
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(Path::new(csv_energy))?;

    reader.records().fold(
        Ok((0, 0, 0.0)), 
        |x, record| -> Result<(usize, usize, f64)> {
            let (line_no, num_cols, sum) = x?;
            let r = record?;

            let num_cols2 = match line_no {
                0 => {
                    if r.len() == 0 { 
                        Err::<(usize, usize, f64), Error>(anyhow!(
                            "price_energy: zero data items on first line of data"
                        ))?; 
                    };
                    r.len()
                },
                _ => {
                    if r.len() != num_cols { 
                        Err::<(usize, usize, f64), Error>(anyhow!(
                            "price_energy: number data items {} on line {} not equal to {} on the first line of data", 
                            r.len(), line_no, num_cols
                        ))?; 
                    };
                    num_cols
                },
            };
            
            let interval = (24 * 60)/(num_cols2 - 1); // 289 for date + 288 data points => 5 minute intervals
            debug!("price_energy: num_cols2 {}, interval {}, record: {:?}", num_cols2, interval, r);
            let date_str = r[0].trim();
            let week_day = match holidays.contains(date_str) {
                true => 6, // if it's a public holiday Sunday=6 tariff applies
                false => {
                    NaiveDate::parse_from_str(date_str, "%Y%m%d")
                    .map(|d| d.weekday().num_days_from_monday() as i16)?
                }
            };
            debug!("price_energy: date_str {}, week_day {}", date_str, week_day);
            
            Ok((
                line_no + 1, 
                num_cols2,
                sum + r.iter().skip(1).enumerate().fold(
                    Ok(0.0),
                    |sum2, (i, energy_str)| -> Result<f64> {
                        let min_since_midnight = (i * interval) as i32;
                        debug!("price_energy: i {}, min_since_midnight {}, energy_str {}", i, min_since_midnight, energy_str);
                        let energy = energy_str.parse::<f64>()?;
                        let t = tariff(week_day, min_since_midnight)?;
                        debug!("price_energy: week_day {}, min_since_midnight {}, energy kWh {}, tariff $/kWh {}", week_day, min_since_midnight, energy, t);
                        Ok(sum2? + t * energy)
                    })?
            ))
    })
}

// very similar to test_price_energy
fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    let daily_supply = load_supply_charge(&args.daily)?;
    
    let holidays = args.public_holidays
    .map(|x| load_public_holidays(&x))
    .unwrap_or_else(|| { Ok(HashSet::new()) })?;
    
    let consumption_tariff = load_tariff(&args.consumption_tariff)?;
    
    let (line_count, _col_count, consumption_cost) = price_energy(
        &args.consumption, 
        |dow, min_since_midnight| lookup_tariff(dow, min_since_midnight, &consumption_tariff),
        &holidays
    )?;

    let (_line_count2, _col_count2, feedin_cost) = match (args.feedin_tariff, args.feedin) {
        (Some(t), Some(e)) => {
            let tarrif = load_tariff(&t)?;
            price_energy(
                &e, 
                |dow, min_since_midnight| lookup_tariff(dow, min_since_midnight, &tarrif),
                &holidays
            )?
        },
        (_, _) => (0, 0, 0.0)
    };

    let supply_cost = line_count as f64 * daily_supply;
    println!("Consumption ${}, Feedin ${}, Supply ${}", consumption_cost, feedin_cost, supply_cost);
    println!("Total ${}", consumption_cost + feedin_cost + supply_cost);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_float_eq::*;

    #[test]
    fn test_minutes_since_midnight() -> Result<()> {
        assert_eq!(minutes_since_midnight("00:00:00")?, 0);
        assert_eq!(minutes_since_midnight("12:34:56")?, 754);
        assert_eq!(minutes_since_midnight("23:59:59")?, 1439);   
        Ok(()) 
    }

    #[test]
    // very similar to main
    fn test_price_energy() -> Result<()> {
        let daily_supply = load_supply_charge(&"data/test/tariff/supply.csv".to_string())?;
        assert_f64_near!(daily_supply, 1.45398);

        let holidays = load_public_holidays(&"data/test/publicHolidaysTest.csv".to_string())?;
        assert!(!holidays.contains("20230807"));
        assert!(holidays.contains("20230808"));
        assert!(holidays.contains("20500101"));

        let consumption_tariff = load_tariff(&"data/test/tariff/consumption.csv".to_string())?;
        // println!("consumption_tariff {:?}", consumption_tariff);
        let (line_count, col_count, consumption_cost) = price_energy(
            &"data/test/energy/consumption.csv".to_string(), 
            |dow, min_since_midnight| lookup_tariff(dow, min_since_midnight, &consumption_tariff),
            &holidays
        )?;
        println!("line_count {}, col_count {}, consumption cost {}", line_count, col_count, consumption_cost);
        assert_f64_near!(consumption_cost, 0.14215773);
    
        let feedin_tariff = load_tariff(&"data/test/tariff/feedIn.csv".to_string())?;
        let (line_count2, col_count2, feedin_cost) = price_energy(
            &"data/test/energy/feedIn.csv".to_string(), 
            |dow, min_since_midnight| lookup_tariff(dow, min_since_midnight, &feedin_tariff),
            &holidays
        )?;
        println!("line_count2 {}, col_count2 {}, feedin cost {}", line_count2, col_count2, feedin_cost);
        assert_f64_near!(feedin_cost, -0.15582);

        let supply_cost = line_count as f64 * daily_supply;
        let total = consumption_cost + feedin_cost + supply_cost;
        println!("total cost {}", total);
        assert_f64_near!(total, 4.34827773);
        Ok(())
    }
}

