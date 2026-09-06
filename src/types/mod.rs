use rust_decimal::Decimal;

pub type ClientId = u16;
pub type TxId = u32;
pub type Amount = Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Deposit {
        client: ClientId,
        tx: TxId,
        amount: Amount,
    },
    Withdrawal {
        client: ClientId,
        tx: TxId,
        amount: Amount,
    },
    Dispute {
        client: ClientId,
        tx: TxId,
    },
    Resolve {
        client: ClientId,
        tx: TxId,
    },
    Chargeback {
        client: ClientId,
        tx: TxId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub(crate) client: ClientId,
    pub(crate) available: Amount,
    pub(crate) held: Amount,
    pub(crate) locked: bool,
}

impl Account {
    pub(crate) fn new(client: ClientId) -> Self {
        Self {
            client,
            available: Amount::ZERO,
            held: Amount::ZERO,
            locked: false,
        }
    }

    pub fn client(&self) -> ClientId {
        self.client
    }

    pub fn available(&self) -> Amount {
        self.available
    }

    pub fn held(&self) -> Amount {
        self.held
    }

    pub fn locked(&self) -> bool {
        self.locked
    }

    pub fn total(&self) -> Amount {
        self.available + self.held
    }
}
