// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

/// @notice Minimal interface for the NativeMinter precompile exposed by the host chain.
interface INativeMinter {
    /// @notice Mints `amount` of native tokens into `account`.
    function mint(address account, uint256 amount) external;

    /// @notice Burns `amount` of native tokens from `account`.
    function burn(address account, uint256 amount) external;
}
