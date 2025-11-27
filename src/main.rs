use futures::{Future, TryStreamExt};
use reth_exex::{ExExContext, ExExEvent, ExExNotification};
use reth_node_api::{BlockBody, FullNodeComponents};
use reth_node_ethereum::EthereumNode;
use alloy_consensus::BlockHeader;
use reth_tracing::tracing::info;

#[derive(Debug, Default, Clone)]
struct TransactionCounter {
    /// Total number of transactions processed
    total_transactions: u64,
    /// Number of blocks processed  
    total_blocks: u64
}

/// The initialization logic of the ExEx is just an async function.
async fn exex_init<Node: FullNodeComponents>(
    ctx: ExExContext<Node>,
) -> eyre::Result<impl Future<Output = eyre::Result<()>>> {
    Ok(exex(ctx))
}

/// An ExEx that counts transactions and blocks.
async fn exex<Node: FullNodeComponents>(mut ctx: ExExContext<Node>) -> eyre::Result<()> {
    let mut counter = TransactionCounter::default();

    while let Some(notification) = ctx.notifications.try_next().await? {
        match &notification {
            ExExNotification::ChainCommitted { new } => {
                // Count blocks and transactions in the committed chain
                for block in new.blocks_iter() {
                    counter.total_blocks += 1;
                    
                    // Count transactions in this block
                    let tx_count = block.body().transactions().len() as u64;
                    counter.total_transactions += tx_count;
                    
                    info!(
                        block_number = block.number(),
                        block_hash = %block.hash(),
                        tx_count,
                        total_transactions = counter.total_transactions,
                        total_blocks = counter.total_blocks,
                        "Processed block"
                    );
                }
                
                info!(
                    committed_chain = ?new.range(),
                    total_transactions = counter.total_transactions,
                    total_blocks = counter.total_blocks,
                    "Processed committed chain"
                );
            }
            ExExNotification::ChainReorged { old, new } => {
                info!(
                    from_chain = ?old.range(),
                    to_chain = ?new.range(),
                    "Received reorg"
                );
                // For simplicity, we just log reorgs
                // In production, we might want to adjust counters
            }
            ExExNotification::ChainReverted { old } => {
                info!(reverted_chain = ?old.range(), "Received revert");
                // For simplicity, we just log reverts
            }
        }

        if let Some(committed_chain) = notification.committed_chain() {
            ctx.events
                .send(ExExEvent::FinishedHeight(committed_chain.tip().num_hash()))?;
        }
    }

    Ok(())
}

fn main() -> eyre::Result<()> {
    reth::cli::Cli::parse_args().run(|builder, _| async move {
        let handle = builder
            .node(EthereumNode::default())
            .install_exex("TransactionCounter", exex_init)
            .launch_with_debug_capabilities()
            .await?;

        handle.wait_for_node_exit().await
    })
}