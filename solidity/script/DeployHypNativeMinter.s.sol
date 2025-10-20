// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

import {Script} from "forge-std/Script.sol";
import {HypNativeMinter} from "../contracts/HypNativeMinter.sol";

/// @notice Deploys HypNativeMinter using parameters supplied via environment variables.
/// - `NATIVE_MINTER_PRECOMPILE`: address of the native token minter precompile.
/// - `HYP_NATIVE_SCALE`: scale factor between Hyperlane tokens and the native token.
/// - `MAILBOX_ADDRESS`: address of the Hyperlane Mailbox contract on this chain.
contract DeployHypNativeMinterScript is Script {
    address internal constant DEFAULT_NATIVE_MINTER_PRECOMPILE =
        0x000000000000000000000000000000000000F100;

    function run() external returns (HypNativeMinter minter) {
        address precompile = vm.envOr(
            "NATIVE_MINTER_PRECOMPILE",
            DEFAULT_NATIVE_MINTER_PRECOMPILE
        );
        uint256 scale = vm.envUint("HYP_NATIVE_SCALE");
        address mailbox = vm.envAddress("MAILBOX_ADDRESS");

        vm.startBroadcast();
        minter = new HypNativeMinter(precompile, scale, mailbox);
        vm.stopBroadcast();
    }
}
