// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

import {Script} from "forge-std/Script.sol";
import {HypNativeMinter} from "../contracts/HypNativeMinter.sol";

/// @notice Deploys HypNativeMinter using parameters supplied via environment variables.
/// - `NATIVE_MINTER_PRECOMPILE`: address of the native token minter precompile.
/// - `HYP_NATIVE_SCALE`: scale factor between Hyperlane tokens and the native token.
/// - `MAILBOX_ADDRESS`: address of the Hyperlane Mailbox contract on this chain.
contract DeployHypNativeMinterScript is Script {
    // INativeToken precompile default address.
    address internal constant _DEFAULT_NATIVE_MINTER_PRECOMPILE =
        0x000000000000000000000000000000000000F100;
    // Default scale used for converting decimal 6 to decimal 18.
    uint256 internal constant _DEFAULT_SCALE = 1e12;

    function run() external returns (HypNativeMinter minter) {
        address mailbox = vm.envAddress("MAILBOX_ADDRESS");
        address precompile = vm.envOr(
            "NATIVE_MINTER_PRECOMPILE",
            _DEFAULT_NATIVE_MINTER_PRECOMPILE
        );
        uint256 scale = vm.envOr("HYP_NATIVE_SCALE", _DEFAULT_SCALE);

        vm.startBroadcast();
        minter = new HypNativeMinter(mailbox, precompile, scale);
        vm.stopBroadcast();
    }
}
