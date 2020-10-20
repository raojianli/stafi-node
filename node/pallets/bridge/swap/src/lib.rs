// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::prelude::*;
use bridge_common::{self as bridge, ResourceId};
use frame_support::traits::{Currency, ExistenceRequirement::AllowDeath, ExistenceRequirement::KeepAlive, Get};
use frame_support::{decl_error, decl_module, dispatch::DispatchResult, ensure};
use frame_system::{self as system, ensure_signed};
use sp_runtime::{traits::{Zero}};
use sp_core::U256;
use sp_arithmetic::traits::SaturatedConversion;
use node_primitives::ChainId;

type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

pub trait Trait: system::Trait + bridge::Trait {
    /// The currency mechanism.
    type Currency: Currency<Self::AccountId>;

    // Ids can be defined by the runtime and passed in, perhaps from blake2b_128 hashes.
    type NativeTokenId: Get<ResourceId>;
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        InvalidChainId,
        InvalidChainFee,
        InsufficientAmount,
        InvalidFeesAccount
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        const NativeTokenId: ResourceId = T::NativeTokenId::get();

        /// Transfers some amount of the native token to some recipient on a (whitelisted) destination chain.
        #[weight = 195_000_000]
        pub fn transfer_native(origin, amount: BalanceOf<T>, recipient: Vec<u8>, dest_id: ChainId) -> DispatchResult {
            let source = ensure_signed(origin)?;
            ensure!(<bridge::Module<T>>::chain_whitelisted(dest_id), Error::<T>::InvalidChainId);

            let chain_fees = <bridge::Module<T>>::get_chain_fees(dest_id)
                .ok_or_else(|| Error::<T>::InvalidChainFee)?;
            let fees: BalanceOf<T> = chain_fees.saturated_into();

            ensure!(amount > fees, Error::<T>::InsufficientAmount);

            let fees_account = <bridge::Module<T>>::get_fees_account()
                .ok_or_else(|| Error::<T>::InvalidFeesAccount)?;

            let swap_amount = amount - fees;

            let bridge_id = <bridge::Module<T>>::account_id();
            T::Currency::transfer(&source, &bridge_id, amount.into(), AllowDeath)?;

            if fees > Zero::zero() {
                T::Currency::transfer(&bridge_id, &fees_account, fees.into(), KeepAlive)?;
            }

            let resource_id = T::NativeTokenId::get();
            <bridge::Module<T>>::transfer_fungible(source, dest_id, resource_id, recipient, U256::from(swap_amount.saturated_into()))
        }
    }
}
