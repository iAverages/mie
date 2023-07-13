package config

import "github.com/spf13/viper"

type Config struct {
	Debug              bool   `mapstructure:"MIE_DEBUG"`
	Port               string `mapstructure:"PORT"`
	Token              string `mapstructure:"MIE_TOKEN"`
	B2ApplicationKeyId string `mapstructure:"B2_APPLICATION_KEY_ID"`
	B2ApplicationKey   string `mapstructure:"B2_APPLICATION_KEY"`
	B2BucketName       string `mapstructure:"B2_BUCKET_NAME"`
	B2BucketPathPrefix string `mapstructure:"B2_BUCKET_PATH_PREFIX"`
	HostUrl            string `mapstructure:"HOST_URL"`
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
