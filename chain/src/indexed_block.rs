use byteorder::{ByteOrder, BE};
use crypto::sha256;
use lazy_static::lazy_static;
use primitives::H256;
use prost::Message;
use proto2::chain::{Block, BlockHeader, Transaction};
use proto2::common::BlockId;
use std::cmp;
use std::collections::HashSet;

use crate::merkle_root::MerkleTree;
use crate::{IndexedBlockHeader, IndexedTransaction};

lazy_static! {
    static ref BLOCK_WHITELIST: HashSet<i64> = {
        use std::iter::FromIterator;

        // 1. malformed `Transaction.ret` field `2a022200`
        //    - `2a` 00101_010, field 5, Length-delimited: `Transaction.ret`
        //    - `02` length = 2
        //    - `2200` error while pasring
        // 2. duplicated `Transaction.ret` field generated by TYVJ8JuQ6ctzCa2u79MFmvvNQ1U2tYQEUM
        HashSet::from_iter(vec![
            1102553, 1103364, 1103650, 1104274, 1104326, 1104948, 1105494, 1106300, 1106888, 1107730, 1110468, 1110780,
            1110832, 1111066, 1111222, 1111430, 1111508, 1111818, 1111896, 1111922, 1111966, 1112021, 1112026, 1112052,
            1112078, 1112099, 1112104, 1112122, 1112130, 1112156, 1112564, 1112668, 1112754, 1113222, 1114106, 1114124,
            1114205, 1114444, 1114522, 1114548, 1114938, 1115536, 1115640, 1115788, 1115822, 1116264, 1116282, 1116308,
            1116386, 1116706, 1116732, 1116758, 1116984, 1117018, 1117460, 1117668, 1117694, 1117928, 1118050, 1118292,
            1118370, 1118604, 1118758, 1118810, 1118914, 1118992, 1119408, 1119460, 1119486, 1119642, 1119660, 1119668,
            1119772, 1120032, 1120084, 1120136, 1120180, 1120344, 1120370, 1120388, 1120422, 1121124, 1121228, 1121402,
            1121696, 1121852, 1122736, 1123022, 1123568, 1123638, 1123750, 1124166, 1124756, 1124990, 1125336, 1125518,
            1125750, 1125846, 1126192, 1126314, 1126340, 1126582, 1127128, 1127536, 1128272, 1129000, 1129858, 1129910,
            1129962, 1130118, 1130690, 1131028, 1131626, 1132922, 1132941, 1132976, 1133084, 1134406, 1135135, 1135513,
            1135675, 1135702, 1135837, 1135918, 1135972,
        ])
    };
}

#[derive(Debug, Clone)]
pub struct IndexedBlock {
    pub header: IndexedBlockHeader,
    pub transactions: Vec<IndexedTransaction>,
}

impl cmp::PartialEq for IndexedBlock {
    fn eq(&self, other: &Self) -> bool {
        self.header.hash == other.header.hash
    }
}

impl IndexedBlock {
    pub fn new(header: IndexedBlockHeader, transactions: Vec<IndexedTransaction>) -> Self {
        IndexedBlock {
            header: header,
            transactions: transactions,
        }
    }

    pub fn from_header_and_txns(header: BlockHeader, txns: Vec<Transaction>) -> Self {
        Self::from_raw(Block {
            block_header: Some(header),
            transactions: txns,
        })
    }

    /// Explicit conversion of the raw Block into IndexedBlock.
    ///
    /// Hashes block header + transactions.
    pub fn from_raw(block: Block) -> Self {
        let Block {
            block_header,
            transactions,
        } = block;
        let transactions = transactions
            .into_iter()
            .map(IndexedTransaction::from_raw)
            .collect::<Vec<_>>();
        let mut block_header = block_header.unwrap();
        if block_header.raw_data.as_ref().unwrap().merkle_root_hash.is_empty() {
            block_header
                .raw_data
                .as_mut()
                .map(|raw| raw.merkle_root_hash = merkle_root(&transactions).as_bytes().to_owned());
        }
        Self::new(IndexedBlockHeader::from_raw(block_header), transactions)
    }

    pub fn hash(&self) -> &H256 {
        &self.header.hash
    }

    pub fn number(&self) -> i64 {
        BE::read_u64(&self.header.hash.as_bytes()[..8]) as i64
    }

    pub fn block_id(&self) -> BlockId {
        BlockId {
            number: self.number(),
            hash: self.hash().as_bytes().to_vec(),
        }
    }

    pub fn into_raw_block(self) -> Block {
        Block {
            block_header: Some(self.header.raw),
            transactions: self.transactions.into_iter().map(|tx| tx.raw).collect(),
        }
    }

    pub fn size(&self) -> usize {
        self.clone().into_raw_block().encoded_len()
    }

    pub fn merkle_root_hash(&self) -> &[u8] {
        &self.header.raw.raw_data.as_ref().unwrap().merkle_root_hash
    }

    pub fn verify_merkle_root_hash(&self) -> bool {
        if BLOCK_WHITELIST.contains(&self.number()) {
            eprintln!(
                "block {} in whitelist, merkle tree match={}",
                self.number(),
                self.merkle_root_hash() == merkle_root(&self.transactions).as_bytes()
            );
            return true;
        }
        if self.merkle_root_hash() == merkle_root(&self.transactions).as_bytes() {
            true
        } else {
            eprintln!("block saved => {:?}", H256::from_slice(self.merkle_root_hash()));
            eprintln!("calculated  => {:?}", merkle_root(&self.transactions));
            false
        }
    }
}

fn merkle_root(transactions: &[IndexedTransaction]) -> H256 {
    let hashes = transactions
        .iter()
        .map(|txn| get_transaction_hash_for_merkle_root(&txn.raw))
        .collect::<Vec<_>>();
    // println!("hashes => {:?}", hashes);
    let tree = MerkleTree::from_vec(hashes);
    *tree.root_hash()
}

fn get_transaction_hash_for_merkle_root(transaction: &Transaction) -> H256 {
    let mut buf = Vec::with_capacity(255);
    // won't fail?
    transaction.encode(&mut buf).unwrap();
    // println!("raw => {:?}", buf);
    sha256(&buf)
}
