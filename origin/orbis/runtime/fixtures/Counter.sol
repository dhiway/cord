// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.20;

contract Counter {
    uint256 public value;

    constructor() {
        value = 41;
    }

    function increment() external {
        value += 1;
    }
}
