use chrono::{DateTime, Duration, Utc};
use solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Signature};
use solana_transaction_status::UiTransactionEncoding;
use std::thread;
use std::time::Duration as StdDuration;
use std::{str::FromStr, time::Instant};

#[derive(Debug)]
struct FeeAnalysis {
    total_transactions: usize,
    successful_transactions: usize,
    failed_transactions: usize,
    total_fees_lamports: u64,
    total_fees_sol: f64,
    average_fee_per_tx: f64,
    time_period: TimePeriod,
}

#[derive(Debug)]
struct TimePeriod {
    from: DateTime<Utc>,
    to: DateTime<Utc>,
}

async fn calculate_fees(
    sender_address: &str,
    hours_to_look_back: i64,
    rpc_endpoint: &str,
) -> Result<FeeAnalysis, Box<dyn std::error::Error>> {
    // Initialize RPC client
    let client =
        RpcClient::new_with_commitment(rpc_endpoint.to_string(), CommitmentConfig::confirmed());

    // Parse sender address
    let sender = Pubkey::from_str(sender_address)?;

    println!("Analyzing transactions for address: {}", sender_address);
    println!("Looking back {} hours from now", hours_to_look_back);

    // Calculate the start time (N hours ago)
    let current_time = Utc::now();
    let start_time = current_time - Duration::hours(hours_to_look_back);

    println!("Start time: {}", start_time.format("%Y-%m-%d %H:%M:%S"));

    // Get signatures for the address
    let mut all_signatures = Vec::new();
    let mut before: Option<Signature> = None;
    let limit = 100;

    loop {
        // Get batch of signatures
        let signatures = client.get_signatures_for_address_with_config(
            &sender,
            GetConfirmedSignaturesForAddress2Config {
                before,
                limit: Some(limit),
                until: None,
                commitment: Some(CommitmentConfig::confirmed()),
            },
        )?;

        if signatures.is_empty() {
            break;
        }

        // Check if we've reached transactions older than our time window
        if let Some(oldest_sig) = signatures.last() {
            if let Some(block_time) = oldest_sig.block_time {
                let oldest_tx_time =
                    DateTime::from_timestamp(block_time, 0).expect("Invalid block time");

                if oldest_tx_time < start_time {
                    // Add any signatures that are within our window
                    for sig in signatures {
                        if let Some(bt) = sig.block_time {
                            let tx_time =
                                DateTime::from_timestamp(bt, 0).expect("Invalid block time");

                            if tx_time >= start_time {
                                all_signatures.push(sig);
                            }
                        }
                    }
                    break;
                }
            }
        }

        all_signatures.extend(signatures.clone());

        // Update 'before' with the oldest signature
        if let Some(oldest_sig) = signatures.last() {
            before = Some(Signature::from_str(&oldest_sig.signature)?);
        }

        // Small delay to avoid rate limiting
        thread::sleep(StdDuration::from_millis(100));
    }

    println!("Retrieved {} total signatures", all_signatures.len());

    // Filter signatures by time
    let filtered_signatures: Vec<_> = all_signatures
        .into_iter()
        .filter(|sig| {
            if let Some(block_time) = sig.block_time {
                let tx_time = DateTime::from_timestamp(block_time, 0).expect("Invalid block time");
                return tx_time >= start_time && tx_time <= current_time;
            }
            false
        })
        .collect();

    println!(
        "Found {} transactions in the specified time period",
        filtered_signatures.len()
    );

    // Get transaction details and calculate fees
    let mut total_fees: u64 = 0;
    let mut processed_tx_count = 0;
    let mut successful_tx_count = 0;
    let mut failed_tx_count = 0;

    // Process in smaller batches to avoid rate limiting
    let batch_size = 5;
    let signature_chunks: Vec<_> = filtered_signatures
        .chunks(batch_size)
        .map(|chunk| chunk.to_vec())
        .collect();

    let timer = Instant::now();

    for (i, chunk) in signature_chunks.iter().enumerate() {
        // Process each signature in the chunk
        let mut chunk_fees = 0;
        let mut chunk_count = 0;

        for sig_info in chunk {
            let sig = Signature::from_str(&sig_info.signature)?;

            // Get transaction details
            match client.get_transaction(&sig, UiTransactionEncoding::Json) {
                Ok(tx) => {
                    if let Some(meta) = tx.transaction.meta {
                        let fee = meta.fee;
                        total_fees += fee;
                        chunk_fees += fee;
                        processed_tx_count += 1;
                        chunk_count += 1;

                        // Check transaction status
                        let status = meta.status.is_ok();
                        if status {
                            successful_tx_count += 1;
                        } else {
                            failed_tx_count += 1;
                        }

                        // Optional: Log compute units if available
                        if meta.compute_units_consumed.is_some() {
                            println!(
                                "Transaction {}: {} lamports, {} compute units, success: {}",
                                processed_tx_count,
                                fee,
                                meta.compute_units_consumed.unwrap_or(0),
                                status
                            );
                        } else {
                            println!(
                                "Transaction {}: {} lamports, success: {}",
                                processed_tx_count, fee, status
                            );
                        }
                    }
                }
                Err(e) => {
                    println!("Error fetching transaction {}: {}", sig, e);
                }
            }

            // Small delay between transactions in a batch
            thread::sleep(StdDuration::from_millis(100));
        }

        // Progress indicator
        println!(
            "Batch {}/{}: Processed {} transactions, {} lamports fees",
            i + 1,
            signature_chunks.len(),
            chunk_count,
            chunk_fees
        );

        println!(
            "Total progress: {}/{} transactions ({}%)",
            processed_tx_count,
            filtered_signatures.len(),
            (processed_tx_count as f64 / filtered_signatures.len() as f64 * 100.0).round()
        );

        // Larger delay between batches
        thread::sleep(StdDuration::from_millis(500));
    }

    // Convert lamports to SOL for final output
    let total_fees_in_sol = total_fees as f64 / 1_000_000_000.0;
    let average_fee = if processed_tx_count > 0 {
        total_fees as f64 / processed_tx_count as f64
    } else {
        0.0
    };

    println!("\n--- SUMMARY ---");
    println!("Total transactions analyzed: {}", processed_tx_count);
    println!("Successful transactions: {}", successful_tx_count);
    println!("Failed transactions: {}", failed_tx_count);
    println!(
        "Success rate: {:.2}%",
        if processed_tx_count > 0 {
            (successful_tx_count as f64 / processed_tx_count as f64) * 100.0
        } else {
            0.0
        }
    );
    println!(
        "Total fees spent: {} lamports ({:.9} SOL)",
        total_fees, total_fees_in_sol
    );
    println!("Average fee per transaction: {:.2} lamports", average_fee);
    println!(
        "Time period: {} to {}",
        start_time.format("%Y-%m-%d %H:%M:%S"),
        current_time.format("%Y-%m-%d %H:%M:%S")
    );
    println!("Analysis completed in {:.2?}", timer.elapsed());

    Ok(FeeAnalysis {
        total_transactions: processed_tx_count,
        successful_transactions: successful_tx_count,
        failed_transactions: failed_tx_count,
        total_fees_lamports: total_fees,
        total_fees_sol: total_fees_in_sol,
        average_fee_per_tx: average_fee,
        time_period: TimePeriod {
            from: start_time,
            to: current_time,
        },
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Command line arguments (or replace with your values)
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        println!(
            "Usage: {} <WALLET_ADDRESS> <HOURS_TO_LOOK_BACK> [RPC_ENDPOINT]",
            args[0]
        );
        println!(
            "Example: {} 7C4jsPZqiKLRQ6JPQcg6V8XMj9os4jHx6iZqBDV7ZJcA 24",
            args[0]
        );
        return Ok(());
    }

    let wallet_address = &args[1];
    let hours: i64 = args[2].parse()?;
    let rpc_endpoint = if args.len() > 3 {
        &args[3]
    } else {
        "https://api.mainnet-beta.solana.com"
    };

    println!("Starting analysis for wallet: {}", wallet_address);

    match calculate_fees(wallet_address, hours, rpc_endpoint).await {
        Ok(analysis) => {
            println!("\nAnalysis complete!");
            println!("Total transactions: {}", analysis.total_transactions);
            println!(
                "Successful transactions: {}",
                analysis.successful_transactions
            );
            println!("Failed transactions: {}", analysis.failed_transactions);
            println!(
                "Success rate: {:.2}%",
                if analysis.total_transactions > 0 {
                    (analysis.successful_transactions as f64 / analysis.total_transactions as f64)
                        * 100.0
                } else {
                    0.0
                }
            );
            println!("Total fees: {:.9} SOL", analysis.total_fees_sol);
            println!(
                "Average fee per tx: {:.2} lamports",
                analysis.average_fee_per_tx
            );
            println!(
                "Time period: {} to {}",
                analysis.time_period.from.format("%Y-%m-%d %H:%M:%S"),
                analysis.time_period.to.format("%Y-%m-%d %H:%M:%S")
            );
            println!("Total fees in lamports: {}", analysis.total_fees_lamports);
        }
        Err(e) => {
            eprintln!("Error during analysis: {}", e);
        }
    }

    Ok(())
}
