#![no_std]

multiversx_sc::imports!();


#[multiversx_sc::contract]
pub trait Counter {
    #[view(get)]
    #[storage_mapper("counter")]
    fn counter(&self) -> SingleValueMapper<Self::Api, u64>;

    #[init]
    fn init(&self, initial_value: u64) {
        self.counter().set(initial_value);
    }

    #[endpoint]
    fn increment(&self) {
        self.counter().update(|val| *val += 1);
    }
}