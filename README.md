# Electricity Costs
## Introduction
Apply a Time of Use (TOU) or fixed tariff to your actual electricity data to calculate the cost. Run again with a different tariff to compare electricity plans.

The following data is required:
1. the tariff plan, your actual cost ($/kWh) depending on:
   - the day (generally week day or week end or public holiday charged as week end)
   - the time of day
   - whether you are buying (consumption) or selling (feed-in)
1. the plan's daily supply charge ($/day)
1. your consumption and feedin data. This is the energy (kWh) that you bought and sold in each small time interval. Australian retailers are required to provide this with at least 30 minute granularty, some provide 5 minute granularity.  
1. dates of public holidays that your tariff plan charges at week end rates 

All the above data needs to be in the form of CSV data files. 
## Build, Test, Run
    cargo build
    cargo test
    ./target/debug/elec --help  # get command line help
    
    for i in data/NB/*Tariff; do
      echo $i;
      ./target/debug/elec \
        --consumption-tariff $i/consumption.csv \
        --feedin-tariff $i/feedIn.csv \
        --daily $i/supply.csv \
        --consumption data/NB/energy/consumption.csv \
        --feedin data/NB/energy/feedIn.csv \
        --public-holidays data/NB/publicHolidaysNSW.csv
    done
    
    data/NB/gloBird2023SingleTariff
    Consumption $253.3821409000001, Feedin $-125.53854999999999, Supply $161.3304
    Total $289.17399090000015
    
    data/NB/gloBird2024TouTariff
    Consumption $345.59194339999993, Feedin $-125.53854999999999, Supply $209.9856
    Total $430.0389934

    data/NB/redEnergy2024SingleTariff
    Consumption $307.12334949999985, Feedin $-175.75396999999998, Supply $174.4776
    Total $305.84697949999986
    
    data/NB/redEnergy2024TouTariff
    Consumption $288.12447630999975, Feedin $-175.75396999999998, Supply $174.4776
    Total $286.84810630999976

## CSV Data Files
### Examples
The data/NB directory contains CSV files with my usage data and plans I'm interested in and NSW public holidays for 2023 and 2024.

The data/test directory contains CSV files used in the unit tests.
### General Requirements
The CSV data files all must have:
 - 1 header line (the content of header line columns is not used)
 - the same number of columns in every line including the header
### Required Files
 - Consumption Tariff file, price ($/kWh)
 - Consumption Data file, your actual energy consumed (kWh)
 - Daily supply charge file, ($/day), although it's only one number, it's in a CSV file just for consistency
### Optional Files
 - Feed-in Tariff file (prices are negative), price ($/kWh)
 - Feed-in Data file, your actual energy exported (kWh)
 - Public holiday file, dates charged as Sundays

The first two are only needed if you receive feed-in credits and the third is only required if your tariff charges public holidays as Sundays.
### Preparation of Files
You'll need to create the tarrif files from information provided by the electrity retailer. When comparing plans make sure to include any available discounts and treat GST consistently.

My retailer provided a single CSV file with a consumption section followed by a feed-in section. All I had to do was to split it into separate consumption and feed-in files.  
