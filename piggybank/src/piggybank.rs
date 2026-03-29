#![no_std]

multiversx_sc::imports!();

/// A piggybank contract that collects funds toward a savings target.
///
/// Demonstrates: require! validation, multiple storage mappers,
/// conditional logic, deposit/withdraw patterns, and error handling.
#[multiversx_sc::contract]
pub trait Piggybank {
    #[view(getTotal)]
    #[storage_mapper("total")]
    fn total(&self) -> SingleValueMapper<Self::Api, u64>;

    #[view(getNumDeposits)]
    #[storage_mapper("num_deposits")]
    fn num_deposits(&self) -> SingleValueMapper<Self::Api, u64>;

    #[view(getTarget)]
    #[storage_mapper("target")]
    fn target(&self) -> SingleValueMapper<Self::Api, u64>;

    #[init]
    fn init(&self, target_amount: u64) {
        self.target().set(target_amount);
    }

    #[endpoint]
    fn deposit(&self, amount: u64) {
        require!(amount > 0, "Deposit must be greater than 0");
        self.total().update(|t| *t += amount);
        self.num_deposits().update(|n| *n += 1);
    }

    #[endpoint]
    fn withdraw(&self, amount: u64) {
        let current = self.total().get();
        require!(current >= amount, "Insufficient funds in piggybank");
        self.total().update(|t| *t -= amount);
    }

    #[view(getStatus)]
    fn get_status(&self) -> u8 {
        if self.total().get() >= self.target().get() {
            1 // target reached
        } else {
            0 // still collecting
        }
    }
}
