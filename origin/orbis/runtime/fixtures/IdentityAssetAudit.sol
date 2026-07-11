// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.20;

interface IERC20 {
    function transfer(address recipient, uint256 amount) external returns (bool);
}

contract IdentityAssetAudit {
    address public immutable owner;
    bytes32 public immutable identityCommitment;
    bytes32 public lastAudit;

    constructor(bytes32 identity) {
        owner = msg.sender;
        identityCommitment = identity;
    }

    function transferAndAudit(
        address asset,
        address recipient,
        uint256 amount,
        bytes32 audit
    ) external {
        require(msg.sender == owner, "identity owner required");
        require(IERC20(asset).transfer(recipient, amount), "asset transfer failed");
        lastAudit = audit;
    }
}
