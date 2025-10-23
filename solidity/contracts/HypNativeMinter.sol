// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

import {INativeToken} from "./interfaces/precompile/INativeToken.sol";
import {TokenRouter} from "@hyperlane-xyz/core/contracts/token/libs/TokenRouter.sol";
import {Address} from "@openzeppelin/contracts/utils/Address.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/security/ReentrancyGuard.sol";

contract HypNativeMinter is TokenRouter, ReentrancyGuard {
    // INativeToken interface for native mint precompile.
    INativeToken public immutable nativeToken;
    // The scale used for decimal conversion of denomations (e.g. decimal 6 -> decimal 18 = 1e12).
    uint256 public immutable scale;
    // Total value locked via the HypNativeMinter bridge supply contract.
    uint256 private _locked;

    error BurnFailed(bytes reason);
    error MintFailed(bytes reason);

    constructor(
        address _mailbox,
        address _precompile,
        uint256 _scale
    ) TokenRouter(_mailbox) {
        nativeToken = INativeToken(_precompile);
        scale = _scale;
    }

    /**
     * @notice Returns the balance of the provided account.
     * @param _account The account address.
     * @return The balance of the account.
     */
    function balanceOf(
        address _account
    ) external view override returns (uint256) {
        return _account.balance;
    }

    /**
     * @notice Returns the total locked supply of the HypNativeMinter.
     * @return The total locked supply.
     */
    function totalLocked() external view returns (uint256) {
        return _locked;
    }

    /**
     * @notice Transfers `_amount` token to `_recipient` on `_destination` domain.
     * @dev Delegates transfer logic to `_transferFromSender` implementation.
     * @param _destination The domain identifier of the destination chain.
     * @param _recipient The address of the recipient on the destination chain.
     * @param _amount The amount or identifier of tokens to be sent to the remote recipient.
     * @return messageId The identifier of the dispatched message.
     */
    function transferRemote(
        uint32 _destination,
        bytes32 _recipient,
        uint256 _amount
    ) public payable virtual override returns (bytes32 messageId) {
        require(
            msg.value >= _amount,
            "HypNativeMinter: amount exceeds msg.value"
        );

        uint256 gasPayment = msg.value - _amount;
        uint256 scaledAmount = _amount / scale;
        require(scaledAmount > 0, "HypNativeMinter: destination amount < 1");

        return
            _transferRemote(_destination, _recipient, scaledAmount, gasPayment);
    }

    /**
     * @notice Calculates the total withdrawal by scaling `_amount` and burning via the INativeToken precompile.
     * @dev Delegates burning to the INativeToken precompile.
     * @dev Decrements the total locked supply.
     * @param _amount The amount of tokens to be sent to the remote recipient.
     * @return Empty, no metadata.
     */
    function _transferFromSender(
        uint256 _amount
    ) internal override returns (bytes memory) {
        uint256 scaledAmount = _amount * scale;
        require(
            _locked >= scaledAmount,
            "HypNativeMinter: amount exceeds total locked value"
        );

        try nativeToken.burn(address(this), scaledAmount) {
            _locked -= scaledAmount;
        } catch (bytes memory reason) {
            revert BurnFailed(reason);
        }

        return bytes(""); // no metadata
    }

    /**
     * @notice Calculates the total deposit by scaling `_amount` and minting via the INativeToken precompile.
     * @dev This method is invoked via the TokenRouter and Mailbox process entrypoint.
     * @dev Delegates minting to the INativeToken precompile.
     * @dev Increments the total locked supply.
     * @param _recipient The address of the recipient on this chain.
     * @param _amount The amount of tokens to be sent to the recipient.
     */
    function _transferTo(
        address _recipient,
        uint256 _amount,
        bytes calldata // no metadata
    ) internal virtual override nonReentrant {
        uint256 scaledAmount = _amount * scale;

        try nativeToken.mint(address(this), scaledAmount) {
            _locked += scaledAmount;
            Address.sendValue(payable(_recipient), scaledAmount);
        } catch (bytes memory reason) {
            revert MintFailed(reason);
        }
    }
}
