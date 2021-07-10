#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure,
};
use sp_std::prelude::*;

use frame_system::{self as system, ensure_root, ensure_signed};
use node_primitives::RSymbol;
use rdex_requestor as requestor;
use rdex_token_price as token_price;

pub trait Trait: system::Trait + requestor::Trait + token_price::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_event! {
    pub enum Event<T> where
        AccountId = <T as system::Trait>::AccountId
    {
        /// rtoken prices vec enough: rsymbol, period_version, period, price
        RTokenPriceEnough(RSymbol, u32, u32, u128),
        /// submit rtoken price: account, rsymbol, period_version, period, price
        SubmitRtokenPrice(AccountId, RSymbol, u32, u32, u128),
        /// fis prices vec enough: period_version, period, price
        FisPriceEnough(u32, u32, u128),
        /// submit fis price: account, period_version, period, price
        SubmitFisPrice(AccountId, u32, u32, u128),
        /// PeriodBlockNumberChanged: period_version, block_number
        PeriodBlockNumberChanged(u32, u32),
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// price duplicated
        PriceRepeated,
        /// price is zero
        PriceZero,
        /// params err
        ParamsErr,
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as RDexOracle {
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        /// Submit rtoken price
        #[weight = 10_000_000]
        pub fn submit_rtoken_price(origin, symbol: RSymbol, period: u32, price: u128) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let period_version = token_price::PeriodVersion::get() as u32;
            // check
            ensure!(requestor::Module::<T>::is_requestor(&who), requestor::Error::<T>::MustBeRequestor);
            ensure!(token_price::AccountRTokenPrices::<T>::get((&who, symbol, period_version, period)).is_none(), Error::<T>::PriceRepeated);
            ensure!(price > u128::MIN, Error::<T>::ParamsErr);
            // update prices vec
            let mut prices = token_price::RTokenPrices::get(symbol, (period_version, period)).unwrap_or(vec![]);
            prices.push(price);
            token_price::RTokenPrices::insert(symbol, (period_version, period), &prices);
            token_price::AccountRTokenPrices::<T>::insert((&who, symbol, period_version, period), price);
            // update CurrentRTokenPrice and HisRTokenPrice
            if prices.len() == requestor::RequestorThreshold::get() as usize {
                prices.sort_by(|a, b| b.cmp(a));
                let will_use_price = prices.get(prices.len() / 2).unwrap_or(&u128::MIN);
                ensure!(*will_use_price > u128::MIN, Error::<T>::PriceZero);

                token_price::CurrentRTokenPrice::insert(symbol, will_use_price);
                token_price::HisRTokenPrice::insert(symbol, (period_version, period), will_use_price);

                //clear unused data
                let data_reserve_period = token_price::DataReservePeriod::get() as u32;
                if period > data_reserve_period {
                    token_price::RTokenPrices::remove(symbol, (period_version, period - data_reserve_period));
                    token_price::AccountRTokenPrices::<T>::remove((&who, symbol, period_version, period - data_reserve_period));
                }
                Self::deposit_event(RawEvent::RTokenPriceEnough(symbol, period_version, period, *will_use_price));
             }

            Self::deposit_event(RawEvent::SubmitRtokenPrice(who.clone(), symbol, period_version, period, price));
            Ok(())
        }
        /// Submit fis price
        #[weight = 10_000_000]
        pub fn submit_fis_price(origin, period: u32, price: u128) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let period_version = token_price::PeriodVersion::get() as u32;
            // check
            ensure!(requestor::Module::<T>::is_requestor(&who), requestor::Error::<T>::MustBeRequestor);
            ensure!(token_price::AccountFisPrices::<T>::get((&who, period_version, period)).is_none(), Error::<T>::PriceRepeated);
            ensure!(price > u128::MIN, Error::<T>::ParamsErr);
            // update prices vec
            let mut prices = token_price::FisPrices::get((period_version, period)).unwrap_or(vec![]);
            prices.push(price);
            token_price::FisPrices::insert((period_version, period), &prices);
            token_price::AccountFisPrices::<T>::insert((&who, period_version, period), price);
            // update CurrentFisPrice and HisFisPrice
            if prices.len() == requestor::RequestorThreshold::get() as usize {
                prices.sort_by(|a, b| b.cmp(a));
                let will_use_price = prices.get(prices.len() / 2).unwrap_or(&u128::MIN);
                ensure!(*will_use_price > u128::MIN, Error::<T>::PriceZero);

                token_price::CurrentFisPrice::put(will_use_price);
                token_price::HisFisPrice::insert((period_version, period), will_use_price);

                //clear unused data
                let data_reserve_period = token_price::DataReservePeriod::get() as u32;
                if period > data_reserve_period {
                    token_price::FisPrices::remove((period_version, period - data_reserve_period));
                    token_price::AccountFisPrices::<T>::remove((&who, period_version, period - data_reserve_period));
                }
                Self::deposit_event(RawEvent::FisPriceEnough(period_version, period, *will_use_price));
             }

            Self::deposit_event(RawEvent::SubmitFisPrice(who.clone(), period_version, period, price));
            Ok(())
        }

        /// set period block number.
        #[weight = 10_000]
        pub fn set_period_block_number(origin, block_number: u32) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(block_number > u32::MIN, Error::<T>::ParamsErr);

            token_price::PeriodBlockNumber::put(block_number);
            token_price::PeriodVersion::mutate(|i| *i += 1);

            Self::deposit_event(RawEvent::PeriodBlockNumberChanged(token_price::PeriodVersion::get() as u32, block_number));
            Ok(())
        }

        /// set data_reserve_period.
        #[weight = 10_000]
        pub fn set_data_reserve_period(origin, reserve_period: u32) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(reserve_period > u32::MIN, Error::<T>::ParamsErr);
            token_price::DataReservePeriod::put(reserve_period);
            Ok(())
        }
    }
}
