use std::net::SocketAddr;

use tla_sm::Acceptor;
use crate::TezedgeState;
use crate::proposals::ExtendPotentialPeersProposal;

impl<P> Acceptor<ExtendPotentialPeersProposal<P>> for TezedgeState
    where P: IntoIterator<Item = SocketAddr>,
{
    fn accept(&mut self, proposal: ExtendPotentialPeersProposal<P>) {
        if let Err(_err) = self.validate_proposal(&proposal) {
            #[cfg(test)]
            assert_ne!(_err, crate::InvalidProposalError::ProposalOutdated);
            return;
        }
        self.extend_potential_peers(proposal.peers.into_iter().map(|x| x.into()));
        self.periodic_react(proposal.at);
    }
}
