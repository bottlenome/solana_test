use borsh::{BorshDeserialize, BorshSerialize};
use thiserror::Error;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

// プログラムデータ
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct BinaryOptionData {
    pub score: u32,
    pub maturity_timestamp: u32,
    pub strike_price: u64,
    pub is_higher: u8,
    pub is_betting: u8,
}

// プログラム引数
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct BinaryOptionInstruction {
    pub command: u32,
}

const MATURITY_MARGIN: u32 = 5;
const SOL_USD_KEY: &str = "FmAmfoyPXiA8Vhhe6MZTr3U6rZfEZ1ctEHay1ysqCqcf";

#[derive(Clone, Debug, Eq, Error, PartialEq)]
enum BinaryOptionError {
    #[error("the maturity has not reached.")]
    MaturityNotReached,
    #[error("price feed is not available.")]
    MarketPriceNotFound,
    #[error("you must bet first.")]
    NoPosition,
}
impl From<BinaryOptionError> for ProgramError {
    fn from(e: BinaryOptionError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

entrypoint!(process_instruction);
// エントリーポイント
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    // クライアントから渡されたアカウントの情報を取得
    let data_account = next_account_info(accounts_iter)?;
    let feed_account = next_account_info(accounts_iter)?;

    if data_account.owner != program_id || feed_account.key.to_string() != String::from(SOL_USD_KEY) {
        return Err(ProgramError::InvalidAccountData);
    }

    let mut program_data: BinaryOptionData = BinaryOptionData::try_from_slice(&data_account.data.borrow())?;

    let clock = Clock::get()?;
    // 引数を処理
    let instruction: BinaryOptionInstruction = BinaryOptionInstruction::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    msg!("コマンド: {}", instruction.command);
    let result: Result<(), ProgramError> = match instruction.command {
        0 => // 結果反映
            if program_data.is_betting == 0 {
                msg!("ポジションがありません");
                Err(BinaryOptionError::NoPosition.into())
            } else if program_data.maturity_timestamp + MATURITY_MARGIN < clock.unix_timestamp as u32 {
                settle(&mut program_data, feed_account)
            } else {
                msg!("満期に達していません");
                Err(BinaryOptionError::MaturityNotReached.into())
            }
        1 | 2 => // ポジション構築
            if program_data.is_betting == 0 {
                let is_higher = if instruction.command == 1 { 1 } else { 0 };
                bet(&mut program_data, is_higher, clock.unix_timestamp as u32, feed_account)
            } else {
                Err(ProgramError::InvalidInstructionData)
            }
        _ => Err(ProgramError::InvalidInstructionData)
    };

    result.and_then(|_| {
        program_data.serialize(&mut &mut data_account.data.borrow_mut()[..])
            .map_err(|e| ProgramError::from(e))
    }).map(|_| ())
}

fn settle(program_data: &mut BinaryOptionData, feed_account: &AccountInfo) -> Result<(), ProgramError> {
    let price = chainlink::get_round(&chainlink::id(), feed_account, program_data.maturity_timestamp as i64)?;
    if let Some(chainlink::state::Submission(ts, settlement_price)) = price {
        msg!("満期時刻: {}", ts);
        msg!("清算価格: {}", settlement_price as u64);
        msg!("行使価格: {}", program_data.strike_price);
        msg!("賭け: {}", if program_data.is_higher == 1 { "上" } else { "下" });
        if program_data.is_higher == 0 && program_data.strike_price > settlement_price as u64
        || program_data.is_higher == 1 && program_data.strike_price < settlement_price as u64 {
            msg!("当たり??ﾌｯ");
            program_data.score += 1;
        } else {
            msg!("外れた??ﾋﾟｴﾝ");
            program_data.score -= 1;
        }
    } else {
        msg!("価格が取得できませんでした??");
        program_data.score -= 1;
    }
    program_data.is_betting = 0;
    Ok(())
}

fn bet(program_data: &mut BinaryOptionData, is_higher: u8, current_timestamp: u32, feed_account: &AccountInfo) -> Result<(), ProgramError> {
    if let Some(current_price) = chainlink::get_price(&chainlink::id(), feed_account)? {
        program_data.strike_price = current_price as u64;
        program_data.maturity_timestamp = current_timestamp + 300; // 満期は5分後
        program_data.is_higher = is_higher;
        program_data.is_betting = 1;
        Ok(())
    } else {
        Err(BinaryOptionError::MarketPriceNotFound.into())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
