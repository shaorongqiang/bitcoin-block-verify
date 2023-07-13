// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.9;

import "openzeppelin/contracts/governance/Governor.sol";
import "openzeppelin/contracts/governance/extensions/GovernorSettings.sol";
import "openzeppelin/contracts/governance/extensions/GovernorVotes.sol";
import "openzeppelin/contracts/governance/extensions/GovernorVotesQuorumFraction.sol";

import {BonsaiLowLevelCallbackReceiver} from "bonsai-lib-sol/BonsaiLowLevelCallbackReceiver.sol";
import {IBonsaiRelay} from "bonsai-lib-sol/IBonsaiRelay.sol";

import "./BonsaiGovernorCounting.sol";
import "./IBonsaiGovernor.sol";

/// @custom:security-contact security@risczero.com
contract BonsaiGovernor is
    IBonsaiGovernor,
    GovernorSettings,
    BonsaiGovernorCounting,
    GovernorVotes,
    GovernorVotesQuorumFraction,
    BonsaiLowLevelCallbackReceiver
{
    /// @notice RISC Zero zkVM image ID for the vote finalization program.
    bytes32 public immutable imageId;

    constructor(IVotes token_, IBonsaiRelay relay_, bytes32 imageId_)
        Governor("BonsaiGovernor")
        GovernorSettings(300, /* blocks */ 21000, /* blocks */ 0)
        GovernorVotes(token_)
        GovernorVotesQuorumFraction(20)
        BonsaiLowLevelCallbackReceiver(relay_)
    {
        imageId = imageId_;
    }

    /**
     * @notice Calculate the current state of the proposal.
     * @dev See {IGovernor-state}.
     */
    function state(uint256 proposalId) public view override(IGovernor, Governor) returns (ProposalState) {
        ProposalState superState = super.state(proposalId);

        // If the votes have not been finalized, by proving the off-chain verified list of validated
        // ballots, then keep the proposal status as active. IGovernor does not provide a state to
        // indicate that voting has ended, but the result is unknown.
        if (superState == ProposalState.Defeated && !_proposalVotesFinalized(proposalId)) {
            return ProposalState.Active;
        }
        return superState;
    }

    /**
     * @dev See {IGovernor-castVote}.
     *      Does not return the voter's balance, since balance lookups are deferred.
     */
    function castVote(uint256 proposalId, uint8 support) public override(Governor, IGovernor) returns (uint256) {
        address voter = _msgSender();
        _commitVote(proposalId, support, voter);
        emit VoteCast(voter, proposalId, support, 0, "");
        return 0;
    }

    /**
     * @dev See {IGovernor-castVoteWithReason}.
     *      Does not return the voter's balance, since balance lookups are deferred.
     */
    function castVoteWithReason(uint256 proposalId, uint8 support, string calldata reason)
        public
        override(Governor, IGovernor)
        returns (uint256)
    {
        address voter = _msgSender();
        _commitVote(proposalId, support, voter);
        emit VoteCast(voter, proposalId, support, 0, reason);
        return 0;
    }

    /*
     * @dev See {IGovernor-castVoteWithReasonAndParams}.
     *      Does not return the voter's balance, since balance lookups are deferred.
     */
    function castVoteWithReasonAndParams(uint256 proposalId, uint8 support, string calldata reason, bytes memory params)
        public
        override(Governor, IGovernor)
        returns (uint256)
    {
        require(params.length == 0, "BonsaiGovernor: params are not supported");

        address voter = _msgSender();
        _commitVote(proposalId, support, voter);
        emit VoteCast(voter, proposalId, support, 0, reason);
        return 0;
    }

    /**
     * @dev See {IGovernor-castVoteBySig}.
     *      Does not return the voter's balance, since balance lookups are deferred.
     *      Also does not log a VoteCast event because it cannot be determined yet if this is a valid vote.
     */
    function castVoteBySig(uint256 proposalId, uint8 support, uint8 v, bytes32 r, bytes32 s)
        public
        override(Governor, IGovernor)
        returns (uint256)
    {
        bytes32 digest = voteHash(proposalId, support);
        _commitVoteBySig(proposalId, support, v, r, s, digest);
        return 0;
    }

    /**
     * @dev See {IGovernor-castVoteWithReasonAndParamsBySig}.
     *      Does not return the voter's balance, since balance lookups are deferred.
     *      Also does not log a VoteCast event because it cannot be determined yet if this is a valid vote.
     */
    function castVoteWithReasonAndParamsBySig(
        uint256 proposalId,
        uint8 support,
        string calldata reason,
        bytes memory params,
        uint8 v,
        bytes32 r,
        bytes32 s
    ) public override(Governor, IGovernor) returns (uint256) {
        require(params.length == 0, "BonsaiGovernor: params are not supported");

        bytes32 digest = voteHashWithReasonAndParamsBySig(proposalId, support, reason, params);
        _commitVoteBySig(proposalId, support, v, r, s, digest);
        return 0;
    }

    /// @notice finalize the vote count, to be called by the zkVM vote finalization program.
    /// @param proposalId the proposal identifier to be finalized.
    /// @param finalBallotBoxAccum hash of the ballot inputs, which must match the proposal.
    /// @param encodedBallots a packed byte array containing the deduplicated list of validated
    ///   ballots is a 24-byte { uint32(support) || address } format. Note that it is provided as
    ///   bytes instead of any decoded form such that it can be processed without copying the
    ///   calldata to memory.
    function finalizeVotes(uint256 proposalId, bytes32 finalBallotBoxAccum, bytes calldata encodedBallots) internal {
        _finalizeVotes(proposalId, finalBallotBoxAccum, encodedBallots);
    }

    /// @notice Upon callback from the Bonsai Relay contract, this function is invoked with the
    /// image ID and journal data.
    /// @dev The image ID must be checked here to match the expected caller.
    function bonsaiLowLevelCallback(bytes calldata journal, bytes32 imageId_)
        internal
        override
        returns (bytes memory)
    {
        require(imageId_ == imageId, "callback does not come from the expected imageId");
        uint256 proposalId = uint256(bytes32(journal[0:32]));
        bytes32 finalBallotBoxAccum = bytes32(journal[32:64]);
        bytes calldata encodedBallots = journal[64:];
        finalizeVotes(proposalId, finalBallotBoxAccum, encodedBallots);
        return new bytes(0);
    }

    function _castVote(uint256, address, uint8, string memory, bytes memory) internal pure override returns (uint256) {
        revert("_castVote is not supported");
    }

    // The following functions are overrides required by Solidity.

    function propose(
        address[] memory targets,
        uint256[] memory values,
        bytes[] memory calldatas,
        string memory description
    ) public override(IGovernor, Governor, BonsaiGovernorCounting) returns (uint256) {
        return super.propose(targets, values, calldatas, description);
    }

    function votingDelay() public view override(IGovernor, GovernorSettings) returns (uint256) {
        return super.votingDelay();
    }

    function votingPeriod() public view override(IGovernor, GovernorSettings) returns (uint256) {
        return super.votingPeriod();
    }

    function quorum(uint256 blockNumber)
        public
        view
        override(IGovernor, GovernorVotesQuorumFraction)
        returns (uint256)
    {
        return super.quorum(blockNumber);
    }

    function proposalThreshold() public view override(Governor, GovernorSettings) returns (uint256) {
        return super.proposalThreshold();
    }
}