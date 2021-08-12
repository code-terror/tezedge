use std::fmt::{self, Debug};
use std::time::Instant;
use tla_sm::recorders::{CloneRecorder, RecordedStream, StreamRecorder};
use tla_sm::{DefaultRecorder, Proposal};

use crate::{EffectsRecorder, PeerAddress, RecordedEffects};

use super::{MaybeRecordedProposal, PeerReadableProposal};

// TODO: there should be difference between 2 kinds of proposals. 1 is
// normal one, which is meant to be recorded and if replayed will give
// us exact same state. and 2nd is like this one, which is purely just
// for performance reasons. Since we invoke read on reader which makes a
// syscall to read certain bytes means that those bytes aren't available
// and are consumed on read, hence hard to debug. So these types of proposals
// should simply invoke another proposal like in this case that the chunk
// is ready so that proposal can be recorded and replayed.
pub struct PeerWritableProposal<'a, Efs, S> {
    pub effects: &'a mut Efs,
    pub at: Instant,
    pub peer: PeerAddress,
    pub stream: &'a mut S,
}

impl<'a, Efs, S> Debug for PeerWritableProposal<'a, Efs, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerWritableProposal")
            .field("at", &self.at)
            .field("peer", &self.peer)
            .finish()
    }
}

impl<'a, Efs, S> Proposal for PeerWritableProposal<'a, Efs, S> {
    fn time(&self) -> Instant {
        self.at
    }
}

impl<'a, Efs, S> From<PeerReadableProposal<'a, Efs, S>> for PeerWritableProposal<'a, Efs, S> {
    fn from(proposal: PeerReadableProposal<'a, Efs, S>) -> Self {
        Self {
            effects: proposal.effects,
            at: proposal.at,
            peer: proposal.peer,
            stream: proposal.stream,
        }
    }
}

impl<'a, Efs, S> DefaultRecorder for PeerWritableProposal<'a, Efs, S> {
    type Recorder = PeerWritableProposalRecorder<'a, Efs, S>;

    fn default_recorder(self) -> Self::Recorder {
        Self::Recorder::new(self)
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct RecordedPeerWritableProposal {
    pub effects: RecordedEffects,
    pub at: Instant,
    pub peer: PeerAddress,
    pub stream: RecordedStream,
}

impl<'a> MaybeRecordedProposal for &'a mut RecordedPeerWritableProposal {
    type Proposal = PeerWritableProposal<'a, RecordedEffects, RecordedStream>;

    fn as_proposal(self) -> Self::Proposal {
        Self::Proposal {
            effects: &mut self.effects,
            at: self.at,
            peer: self.peer,
            stream: &mut self.stream,
        }
    }
}

pub struct PeerWritableProposalRecorder<'a, Efs, S> {
    effects: EffectsRecorder<'a, Efs>,
    at: CloneRecorder<Instant>,
    peer: CloneRecorder<PeerAddress>,
    stream: StreamRecorder<&'a mut S>,
}

impl<'a, Efs, S> PeerWritableProposalRecorder<'a, Efs, S> {
    pub fn new(proposal: PeerWritableProposal<'a, Efs, S>) -> Self {
        Self {
            effects: EffectsRecorder::new(proposal.effects),
            at: proposal.at.default_recorder(),
            peer: proposal.peer.default_recorder(),
            stream: StreamRecorder::new(proposal.stream),
        }
    }

    pub fn record<'b>(
        &'b mut self,
    ) -> PeerWritableProposal<'b, EffectsRecorder<'a, Efs>, StreamRecorder<&'a mut S>> {
        PeerWritableProposal {
            effects: self.effects.record(),
            at: self.at.record(),
            peer: self.peer.record(),
            stream: self.stream.record(),
        }
    }

    pub fn finish_recording(self) -> RecordedPeerWritableProposal {
        RecordedPeerWritableProposal {
            effects: self.effects.finish_recording(),
            at: self.at.finish_recording(),
            peer: self.peer.finish_recording(),
            stream: self.stream.finish_recording(),
        }
    }
}
