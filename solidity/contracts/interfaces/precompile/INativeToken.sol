// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

/// @title INativeToken
/// @notice Interface for a native token contract with mint/burn and allowlist control.
interface INativeToken {
    /// @notice Mint tokens to an address.
    /// @param to The address to receive the minted tokens.
    /// @param amount The amount of tokens to mint.
    function mint(address to, uint256 amount) external;

    /// @notice Burn tokens from an address.
    /// @param from The address from which tokens will be burned.
    /// @param amount The amount of tokens to burn.
    function burn(address from, uint256 amount) external;

    /// @notice Add an address to the allow list.
    /// @param account The address to add.
    function addToAllowList(address account) external;

    /// @notice Remove an address from the allow list.
    /// @param account The address to remove.
    function removeFromAllowList(address account) external;
}
