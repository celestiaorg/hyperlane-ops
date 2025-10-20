// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

import {INativeToken} from "./interfaces/precompile/INativeToken.sol";
import {TokenRouter} from "@hyperlane-xyz/core/contracts/token/libs/TokenRouter.sol";
import {Address} from "@openzeppelin/contracts/utils/Address.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/security/ReentrancyGuard.sol";

contract HypNativeMinter is TokenRouter, ReentrancyGuard {
    // solhint-disable
    INativeToken public immutable nativeToken;
    uint256 public immutable scale;
    // solhint-enable

    uint256 private _locked;

    // solhint-disable no-unused-vars
    constructor(
        address _precompile,
        uint256 _scale,
        address _mailbox
    ) TokenRouter(_mailbox) {
        nativeToken = INativeToken(_precompile);
        scale = _scale;
    }
    // solhint-enable no-unused-vars

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

    function balanceOf(
        address _account
    ) external view override returns (uint256) {
        return _account.balance;
    }

    function _transferFromSender(
        uint256 _amount
    ) internal override returns (bytes memory) {
        uint256 scaledAmount = _amount * scale;
        require(
            _locked >= scaledAmount,
            "HypNativeMinter: amount exceeds locked"
        );
        nativeToken.burn(address(this), scaledAmount);
        _locked -= scaledAmount;
        return bytes(""); // no metadata
    }

    function _transferTo(
        address _recipient,
        uint256 _amount,
        bytes calldata // no metadata
    ) internal virtual override nonReentrant {
        uint256 scaledAmount = _amount * scale;
        nativeToken.mint(address(this), scaledAmount);
        _locked += scaledAmount;
        Address.sendValue(payable(_recipient), scaledAmount);
    }
}
