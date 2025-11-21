// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "@hyperlane-xyz/core/contracts/interfaces/IMailbox.sol";
import "@hyperlane-xyz/core/contracts/interfaces/IInterchainSecurityModule.sol";
import "@hyperlane-xyz/core/contracts/interfaces/hooks/IPostDispatchHook.sol";
import "@hyperlane-xyz/core/contracts/libs/TypeCasts.sol";
import "../contracts/HypNativeMinter.sol";
import "../contracts/interfaces/precompile/INativeToken.sol";

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

contract HypNativeMinterHarness is HypNativeMinter {
    constructor(
        address mailbox,
        address precompile,
        uint256 _scale
    ) HypNativeMinter(mailbox, precompile, _scale) {}

    receive() external payable {}

    function exposedTransferFromSender(
        uint256 amount
    ) external returns (bytes memory) {
        return _transferFromSender(amount);
    }

    function exposedTransferTo(
        address recipient,
        uint256 amount,
        bytes calldata metadata
    ) external {
        _transferTo(recipient, amount, metadata);
    }
}

contract HypNativeMinterTest is Test {
    using TypeCasts for address;

    MockMailbox private mailbox;
    MockNativeToken private nativeToken;
    HypNativeMinterHarness private minter;

    address private constant REMOTE_ROUTER = address(uint160(0xc0ffee));
    address private constant LOCAL_RECIPIENT = address(uint160(0xbeef));
    address private constant REMOTE_RECIPIENT = address(uint160(0xb0b));
    uint32 private constant DESTINATION_DOMAIN = 7;
    uint256 private constant SCALE = 1e12;

    function setUp() external {
        mailbox = new MockMailbox(1);
        nativeToken = new MockNativeToken();
        vm.deal(address(nativeToken), 100 ether);
        vm.deal(address(this), 10 ether);

        minter = new HypNativeMinterHarness(
            address(mailbox),
            address(nativeToken),
            SCALE
        );
        minter.enrollRemoteRouter(
            DESTINATION_DOMAIN,
            REMOTE_ROUTER.addressToBytes32()
        );
    }

    function testTransferToMintsAndUpdatesLocked() external {
        uint256 amount = 2;
        uint256 expectedScaled = amount * SCALE;
        uint256 recipientBefore = LOCAL_RECIPIENT.balance;

        minter.exposedTransferTo(LOCAL_RECIPIENT, amount, bytes(""));

        assertEq(nativeToken.lastMintTo(), address(minter));
        assertEq(nativeToken.lastMintAmount(), expectedScaled);
        assertEq(minter.totalLocked(), expectedScaled);
        assertEq(LOCAL_RECIPIENT.balance, recipientBefore + expectedScaled);
    }

    function testTransferRemoteBurnsAndDispatches() external {
        uint256 inboundAmount = 3;
        uint256 lockedAmount = inboundAmount * SCALE;
        minter.exposedTransferTo(LOCAL_RECIPIENT, inboundAmount, bytes(""));

        uint256 burnAmount = SCALE;
        uint256 gasPayment = 1;
        bytes32 messageId = minter.transferRemote{value: burnAmount + gasPayment}(
            DESTINATION_DOMAIN,
            REMOTE_RECIPIENT.addressToBytes32(),
            burnAmount
        );

        assertTrue(messageId != bytes32(0));
        assertEq(nativeToken.lastBurnFrom(), address(minter));
        assertEq(nativeToken.lastBurnAmount(), burnAmount);
        assertEq(minter.totalLocked(), lockedAmount - burnAmount);

        MockMailbox.DispatchRecord memory record = mailbox.getLastDispatch();
        assertEq(record.destination, DESTINATION_DOMAIN);
        assertEq(record.recipient, REMOTE_ROUTER.addressToBytes32());
        assertEq(record.value, gasPayment);
        assertEq(record.tokenRecipient, REMOTE_RECIPIENT.addressToBytes32());
        assertEq(record.tokenAmount, burnAmount / SCALE);
    }

    function testTransferRemoteRevertsWhenValueTooLow() external {
        uint256 burnAmount = SCALE;
        vm.expectRevert(
            bytes("HypNativeMinter: amount exceeds msg.value")
        );
        minter.transferRemote{value: burnAmount - 1}(
            DESTINATION_DOMAIN,
            REMOTE_RECIPIENT.addressToBytes32(),
            burnAmount
        );
    }

    function testTransferRemoteRevertsWhenScaledAmountIsZero() external {
        uint256 burnAmount = SCALE - 1;
        vm.expectRevert(
            bytes("HypNativeMinter: destination amount < 1")
        );
        minter.transferRemote{value: burnAmount}(
            DESTINATION_DOMAIN,
            REMOTE_RECIPIENT.addressToBytes32(),
            burnAmount
        );
    }

    function testTransferFromSenderRevertsWhenLockedTooLow() external {
        vm.expectRevert(
            bytes("HypNativeMinter: amount exceeds total locked value")
        );
        minter.exposedTransferFromSender(1);
    }

    function testTransferToPropagatesMintFailure() external {
        bytes memory revertData = abi.encodeWithSignature(
            "Error(string)",
            "mint fail"
        );
        nativeToken.setMintRevertData(revertData);

        vm.expectRevert(
            abi.encodeWithSelector(
                HypNativeMinter.MintFailed.selector,
                revertData
            )
        );
        minter.exposedTransferTo(LOCAL_RECIPIENT, 1, bytes(""));
    }

    function testTransferFromSenderPropagatesBurnFailure() external {
        minter.exposedTransferTo(LOCAL_RECIPIENT, 1, bytes(""));

        bytes memory revertData = abi.encodeWithSignature(
            "Error(string)",
            "burn fail"
        );
        nativeToken.setBurnRevertData(revertData);

        vm.expectRevert(
            abi.encodeWithSelector(
                HypNativeMinter.BurnFailed.selector,
                revertData
            )
        );
        minter.exposedTransferFromSender(1);
    }
}
