package parachain

import "github.com/snowfork/snowbridge/relayer/config"

type Config struct {
	Source SourceConfig `mapstructure:"source"`
	Sink   SinkConfig   `mapstructure:"sink"`
}

type SourceConfig struct {
	Substrate config.PolkadotConfig `mapstructure:"substrate"`
	Ethereum  config.EthereumConfig `mapstructure:"ethereum"`
	Contracts SourceContractsConfig `mapstructure:"contracts"`
}

type SourceContractsConfig struct {
	BeefyLightClient           string `mapstructure:"BeefyLightClient"`
	BasicInboundChannel        string `mapstructure:"BasicInboundChannel"`
	IncentivizedInboundChannel string `mapstructure:"IncentivizedInboundChannel"`
}

type SinkConfig struct {
	Ethereum  config.EthereumConfig `mapstructure:"ethereum"`
	Contracts SinkContractsConfig   `mapstructure:"contracts"`
}

type SinkContractsConfig struct {
	BasicInboundChannel        string `mapstructure:"BasicInboundChannel"`
	IncentivizedInboundChannel string `mapstructure:"IncentivizedInboundChannel"`
}
