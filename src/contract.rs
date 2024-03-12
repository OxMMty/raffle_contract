#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_json_binary, CosmosMsg, WasmMsg, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, BankMsg, coin, QuerierWrapper, WasmQuery, QueryRequest, Addr};
use cw2::set_contract_version;
use cw721::Cw721ExecuteMsg;
use sha2::{Sha256, Digest};

use cw721::{Cw721QueryMsg, OwnerOfResponse}; 
// use cosmwasm_std::{to_json_binary, Addr, QuerierWrapper, StdResult, WasmQuery, QueryRequest};

use crate::error::ContractError;
use crate::msg::{RaffleResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{State, TicketInfo, STATE, TICKET_STATUS};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:raffle";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let sender_str = info.sender.clone().to_string();
    let data_to_hash = format!("{}{}", sender_str, "sei1j7ah3st8qjr792qjwtnjmj65rqhpedjqf9dnsddj");
    let mut hasher = Sha256::new();
    hasher.update(data_to_hash.as_bytes());
    let result_hash = hasher.finalize();
    let hex_encoded_hash = hex::encode(result_hash);

    // Compare the generated hash with `msg.authkey`
    if hex_encoded_hash != msg.authkey {
        return Err(ContractError::Unauthorized {});
    }

    let state: State = State {
        ticket_price: 0,
        sold_ticket_count: 0,
        total_ticket_count: 0,
        expected_participants_count: 0,
        raffle_status: 0, // Assuming 0 represents 'not started'
        nft_contract_addr: None, // Initialized with empty string
        nft_token_id: "".to_string(), // Initialized with empty string
        count: msg.count.clone(),
        owner: msg.owner.clone(),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", msg.owner.to_string())
        .add_attribute("count", msg.count.to_string())
        .add_attribute("contract_address", env.contract.address.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::ReceiveNft { sender, token_id, msg } => try_receive_nft(deps, env, info, sender, token_id, msg),
        ExecuteMsg::StartRaffle { ticket_price, total_ticket_count, expected_participants_count, nft_contract_addr, nft_token_id } => 
            try_start_raffle(deps, env, info, ticket_price, total_ticket_count, expected_participants_count, nft_contract_addr, nft_token_id),
        ExecuteMsg::EnterRaffle {} => try_enter_raffle(deps, env, info),
        ExecuteMsg::TransferTokensToCollectionWallet { amount, denom, collection_wallet_address } => try_transfer_tokens_to_collection_wallet(deps, env, info, amount, denom, collection_wallet_address),
        ExecuteMsg::SelectWinnerAndTransferNFTtoWinner {} => try_select_winner_and_transfer_nft_to_winner(deps, env, info),
    }
}

// Pseudo-code for CW721 receiver function
pub fn try_receive_nft(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    // Parameters might include the sender address, token ID, and any additional data
    _sender: String,
    token_id: String,
    _msg: Binary,
) -> Result<Response, ContractError> {

    // Logic to handle the received NFT, such as setting it as the prize for the raffle
    let mut state = STATE.load(deps.storage)?;
    state.nft_contract_addr = Some(info.sender);
    state.nft_token_id = token_id.clone();
    STATE.save(deps.storage, &state)?;

    // Additional logic as necessary, for example, parsing `msg` for any specific instructions

    Ok(Response::new().add_attribute("action", "receive_nft").add_attribute("token_id", token_id))
}

fn can_transfer_nft(querier: &QuerierWrapper, nft_contract_addr: Addr, nft_token_id: String, operator: Addr) -> StdResult<bool> {
    // Adjusted query to fetch ownership information
    let owner_response: OwnerOfResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: nft_contract_addr.into_string(),
        msg: to_json_binary(&Cw721QueryMsg::OwnerOf {
            token_id: nft_token_id,
            // Include field for including expired items or not, based on your contract's requirements
            include_expired: None, // This parameter depends on your CW721 version's API
        })?,
    }))?;

    // Check if the contract is the owner or has been approved
    Ok(owner_response.owner == operator || owner_response.approvals.iter().any(|approval| approval.spender == operator))
}

fn try_start_raffle(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    ticket_price: u32,
    total_ticket_count: u32,
    expected_participants_count: u32,
    nft_contract_addr: Addr,
    nft_token_id: String,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;
    // Check
    if info.sender != state.owner {
        return Err(ContractError::Unauthorized {  });
    }

    if state.raffle_status != 0 {
        return Err(ContractError::RaffleStarted {  });
    }

    if !can_transfer_nft(&deps.querier, nft_contract_addr.clone(), nft_token_id.clone(), env.contract.address)? {
        return Err(ContractError::CantAccessPrize {});
    }
    
    let count_tmp = state.count.clone() + 1;

    // Assuming 1 represents 'active'
    state.raffle_status = 1;
    state.sold_ticket_count = 0; // Reset sold ticket count if necessary
    state.ticket_price = ticket_price;
    state.total_ticket_count = total_ticket_count;
    state.expected_participants_count = expected_participants_count;
    state.nft_contract_addr = Some(nft_contract_addr);
    state.nft_token_id = nft_token_id;
    state.count = count_tmp;
    
    STATE.save(deps.storage, &state)?;
    
    Ok(Response::new().add_attribute("method", "start_raffle").add_attribute("status", "active"))
}

fn try_enter_raffle(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;

    // Ensure the raffle is active
    if state.raffle_status != 1 {
        return Err(ContractError::RaffleNotActive {});
    }

    // Ensure the sold_ticket_count does not exceed total_ticket_count
    if state.sold_ticket_count >= state.total_ticket_count {
        return Err(ContractError::RaffleSoldOut {});
    }

    // Creating a variable with the specified wallet address and count
    let ticket_info = TicketInfo {
        wallet_address: info.sender.clone(),
        count: state.count,
    };

    // Simulate ticket purchase by verifying sent funds match the ticket price
    let ticket_price = state.ticket_price as u128;
    let sent_funds = info.funds.iter().find(|coin| coin.denom == "usei").map_or(0u128, |coin| coin.amount.u128());
    if sent_funds.clone() < ticket_price.clone() {
        return Err(ContractError::IncorrectFunds {});
    }
    let purchase_ticket_count = sent_funds.clone() / ticket_price.clone();
    let real_purchase_ticket_count = std::cmp::min(purchase_ticket_count, state.total_ticket_count.clone() as u128 - state.sold_ticket_count.clone() as u128);
    let start_ticket_number = state.sold_ticket_count.clone();
    // Increment the sold_ticket_count and save the participant's address
    for i in 0..real_purchase_ticket_count{
        TICKET_STATUS.save(deps.storage, start_ticket_number.clone() + i as u32 , &ticket_info)?;
    }
    state.sold_ticket_count += real_purchase_ticket_count.clone() as u32;
    STATE.save(deps.storage, &state)?;

    let refund_amount = sent_funds.clone() - ticket_price * real_purchase_ticket_count.clone();

    if refund_amount > 0 {
        let send_msg = BankMsg::Send {
            to_address: info.sender.into_string(),
            amount: vec![coin(refund_amount, "usei")]
        };
        Ok(Response::new().add_attribute("action", "enter_raffle")
            .add_attribute("start_ticket_number", (start_ticket_number + 1).to_string())
            .add_attribute("purchase_ticket_count", real_purchase_ticket_count.to_string())
            .add_message(send_msg)
        )                
    }
    else{
        Ok(Response::new().add_attribute("action", "enter_raffle")
            .add_attribute("start_ticket_number", (start_ticket_number + 1).to_string())
            .add_attribute("purchase_ticket_count", real_purchase_ticket_count.to_string()))
    }
}

fn try_transfer_tokens_to_collection_wallet(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: u128, // Amount of tokens to transfer
    denom: String, // Token denomination, e.g., "usei" for micro SEI tokens
    collection_wallet_address: String, // Address of the collection wallet
) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;
    let collection_wallet = collection_wallet_address.clone();
    // Authorization check: Ensure the caller is the owner
    if info.sender != state.owner {
        return Err(ContractError::Unauthorized {  });
    }

    if state.raffle_status.clone() == 1 {
        return Err(ContractError::CantTransferTokens {});
    }

    // Create the message to transfer tokens
    let send_msg = BankMsg::Send {
        to_address: collection_wallet_address,
        amount: vec![coin(amount, denom)],
    };

    // Create and return the response that sends the tokens
    Ok(Response::new()
        .add_message(send_msg)
        .add_attribute("action", "transfer_tokens")
        .add_attribute("amount", amount.to_string())
        .add_attribute("to", collection_wallet))
}

fn try_select_winner_and_transfer_nft_to_winner(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;
    let check = state.raffle_status.clone();

    if check == 0 {
        return Err(ContractError::RaffleNotActive {  })
    }

    if info.sender != state.owner {
        return Err(ContractError::Unauthorized {});
    }

    if state.sold_ticket_count.clone() == 0 {
        return Err(ContractError::NoParticipants {});
    }

    if state.sold_ticket_count.clone() < state.expected_participants_count {
        return Err(ContractError::CantFinishRaffle {});
    }

    let mod_number = state.total_ticket_count as u64;
    let sold_count = state.sold_ticket_count as u64;
    let seed_assist = sold_count % mod_number.clone() * (env.block.time.nanos() / 1024 / mod_number.clone() + env.block.height.clone() % mod_number.clone() * 256 % mod_number.clone() + 1) % mod_number.clone();
    let seed = (env.block.time.nanos() % mod_number + env.block.height + seed_assist) % mod_number;
    let winner_index = seed % mod_number;

    // Check if the winner's ticket was actually sold
    match TICKET_STATUS.load(deps.storage, winner_index.clone() as u32) {
        Ok(winner_ticket) => {
            if winner_ticket.count == state.count {

                let transfer_msg = Cw721ExecuteMsg::TransferNft {
                    recipient: winner_ticket.wallet_address.clone().into_string(),
                    token_id: state.nft_token_id.clone(),
                };
    
                let contract_addr = match &state.nft_contract_addr {
                    Some(addr) => addr,
                    None => return Err(ContractError::MissingNftContractAddr{}), // Define this error if it doesn't exist
                };
    
                let msg = CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.clone().into_string(),
                    msg: to_json_binary(&transfer_msg)?,
                    funds: vec![],
                });
    
                // Update the state before returning the response
                state.raffle_status = 0; // End the raffle by setting the status to 0
                STATE.save(deps.storage, &state)?;
    
                let contract_addr = match &state.nft_contract_addr {
                    Some(addr) => addr,
                    None => return Err(ContractError::MissingNftContractAddr{}), // Define this error if it doesn't exist
                };
    
                // Return a response with the winner information and the transfer message
                Ok(Response::new()
                    .add_message(msg)
                    .add_attribute("action", "select_winner_and_transfer_nft")
                    .add_attribute("winner_ticket", (winner_index + 1).to_string())
                    .add_attribute("winner", winner_ticket.wallet_address.into_string())
                    .add_attribute("nft_contract_addr", contract_addr)
                    .add_attribute("token_id", state.nft_token_id))
            }

            else {
                state.raffle_status = 0; // End the raffle
                STATE.save(deps.storage, &state)?;
    
                Ok(Response::new()
                    .add_attribute("action", "select_winner")
                    .add_attribute("winner_ticket", (winner_index + 1).to_string())
                    .add_attribute("status", "Winner ticket was not sold"))
            }
            
        },
        Err(_) => {
            // If the ticket wasn't sold, simply end the raffle without transferring the NFT
            state.raffle_status = 0; // End the raffle
            STATE.save(deps.storage, &state)?;

            Ok(Response::new()
                .add_attribute("action", "select_winner")
                .add_attribute("winner_ticket", (winner_index + 1).to_string())
                .add_attribute("status", "Winner ticket was not sold"))
        }
    }
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetRaffle {} => to_json_binary(&query_raffle(deps)?),
    }
}

fn query_raffle(deps: Deps) -> StdResult<RaffleResponse> {
    let state = STATE.load(deps.storage)?;

    Ok(RaffleResponse { 
        ticket_price: state.ticket_price,
        sold_ticket_count: state.sold_ticket_count,
        total_ticket_count: state.total_ticket_count,
        expected_participants_count: state.expected_participants_count,
        raffle_status: state.raffle_status,
        nft_contract_addr: state.nft_contract_addr,
        nft_token_id: state.nft_token_id,
        count: state.count,
        owner: state.owner
    })
}
