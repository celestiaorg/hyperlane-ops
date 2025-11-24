// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

import "@hyperlane-xyz/core/contracts/interfaces/IMailbox.sol";
import "@hyperlane-xyz/core/contracts/interfaces/IInterchainSecurityModule.sol";
import "@hyperlane-xyz/core/contracts/interfaces/hooks/IPostDispatchHook.sol";
import "../../contracts/interfaces/precompile/INativeToken.sol";

contract MockNativeToken is INativeToken {
    address public lastMintTo;
    uint256 public lastMintAmount;
    address public lastBurnFrom;
    uint256 public lastBurnAmount;

    bytes public mintRevertData;
    bytes public burnRevertData;

    receive() external payable {}

    function setMintRevertData(bytes memory data) external {
        mintRevertData = data;
    }

    function setBurnRevertData(bytes memory data) external {
        burnRevertData = data;
    }

    function mint(address to, uint256 amount) external override {
        if (mintRevertData.length > 0) {
            _revertWith(mintRevertData);
        }

        lastMintTo = to;
        lastMintAmount = amount;

        (bool success, ) = payable(to).call{value: amount}("");
        require(success, "mock mint transfer failed");
    }

    function burn(address from, uint256 amount) external override {
        if (burnRevertData.length > 0) {
            _revertWith(burnRevertData);
        }
        lastBurnFrom = from;
        lastBurnAmount = amount;
    }

    function addToAllowList(address) external pure override {}

    function removeFromAllowList(address) external pure override {}

    function _revertWith(bytes memory data) internal pure {
        assembly {
            revert(add(data, 32), mload(data))
        }
    }
}

contract MockMailbox is IMailbox {
    uint32 public immutable override localDomain;
    bytes32 public override latestDispatchedId;

    struct DispatchRecord {
        uint32 destination;
        bytes32 recipient;
        bytes message;
        bytes hookMetadata;
        address hook;
        uint256 value;
        bytes32 tokenRecipient;
        uint256 tokenAmount;
    }

    DispatchRecord public lastDispatch;

    constructor(uint32 _localDomain) {
        localDomain = _localDomain;
    }

    function _recordDispatch(
        uint32 destinationDomain,
        bytes32 recipientAddress,
        bytes calldata body,
        bytes memory hookMetadata,
        IPostDispatchHook hook
    ) internal returns (bytes32 messageId) {
        (bytes32 parsedRecipient, uint256 parsedAmount) = _decodeTokenMessage(
            body
        );

        lastDispatch = DispatchRecord({
            destination: destinationDomain,
            recipient: recipientAddress,
            message: body,
            hookMetadata: hookMetadata,
            hook: address(hook),
            value: msg.value,
            tokenRecipient: parsedRecipient,
            tokenAmount: parsedAmount
        });
        messageId = keccak256(
            abi.encodePacked(
                block.timestamp,
                destinationDomain,
                recipientAddress,
                body,
                hookMetadata,
                hook,
                msg.value
            )
        );
        latestDispatchedId = messageId;
    }

    function _decodeTokenMessage(
        bytes calldata body
    ) internal pure returns (bytes32 recipient, uint256 amount) {
        require(body.length >= 64, "mock: message too short");
        assembly {
            recipient := calldataload(body.offset)
            amount := calldataload(add(body.offset, 32))
        }
    }

    function getLastDispatch()
        external
        view
        returns (DispatchRecord memory)
    {
        return lastDispatch;
    }

    function dispatch(
        uint32 destinationDomain,
        bytes32 recipientAddress,
        bytes calldata messageBody
    ) external payable override returns (bytes32 messageId) {
        return
            _recordDispatch(
                destinationDomain,
                recipientAddress,
                messageBody,
                bytes(""),
                IPostDispatchHook(address(0))
            );
    }

    function dispatch(
        uint32 destinationDomain,
        bytes32 recipientAddress,
        bytes calldata body,
        bytes calldata hookMetadata
    ) external payable override returns (bytes32 messageId) {
        return
            _recordDispatch(
                destinationDomain,
                recipientAddress,
                body,
                hookMetadata,
                IPostDispatchHook(address(0))
            );
    }

    function dispatch(
        uint32 destinationDomain,
        bytes32 recipientAddress,
        bytes calldata body,
        bytes calldata hookMetadata,
        IPostDispatchHook hook
    ) public payable override returns (bytes32 messageId) {
        return
            _recordDispatch(
                destinationDomain,
                recipientAddress,
                body,
                hookMetadata,
                hook
            );
    }

    function quoteDispatch(
        uint32,
        bytes32,
        bytes calldata
    ) external pure override returns (uint256 fee) {
        return 0;
    }

    function quoteDispatch(
        uint32,
        bytes32,
        bytes calldata,
        bytes calldata
    ) external pure override returns (uint256 fee) {
        return 0;
    }

    function quoteDispatch(
        uint32,
        bytes32,
        bytes calldata,
        bytes calldata,
        IPostDispatchHook
    ) external pure override returns (uint256 fee) {
        return 0;
    }

    function delivered(bytes32) external pure override returns (bool) {
        return false;
    }

    function defaultIsm()
        external
        pure
        override
        returns (IInterchainSecurityModule)
    {
        return IInterchainSecurityModule(address(0));
    }

    function defaultHook()
        external
        pure
        override
        returns (IPostDispatchHook)
    {
        return IPostDispatchHook(address(0));
    }

    function requiredHook()
        external
        pure
        override
        returns (IPostDispatchHook)
    {
        return IPostDispatchHook(address(0));
    }

    function process(bytes calldata, bytes calldata) external payable override {}

    function recipientIsm(
        address
    ) external pure override returns (IInterchainSecurityModule module) {
        return IInterchainSecurityModule(address(0));
    }
}
