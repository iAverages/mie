package config

import "github.com/spf13/viper"

type Config struct {
	Debug bool   `mapstructure:"MIE_DEBUG"`
	Port  string `mapstructure:"PORT"`
}

func LoadConfig(path string) (config Config, err error) {
	viper.AddConfigPath(path)
	viper.SetConfigName("app")
	viper.SetConfigType("env")
	viper.AutomaticEnv()

	// Default values
	viper.SetDefault("MIE_DEBUG", false)
	viper.SetDefault("PORT", "8000")

	err = viper.ReadInConfig()
	if err != nil {
		return
	}

	err = viper.Unmarshal(&config)
	return
}
