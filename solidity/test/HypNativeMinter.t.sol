// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "@hyperlane-xyz/core/contracts/interfaces/IMailbox.sol";
import "@hyperlane-xyz/core/contracts/interfaces/IInterchainSecurityModule.sol";
import "@hyperlane-xyz/core/contracts/interfaces/hooks/IPostDispatchHook.sol";
import "@hyperlane-xyz/core/contracts/libs/TypeCasts.sol";
import "../contracts/HypNativeMinter.sol";
import "./mocks/HypNativeMinterMocks.sol";

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
