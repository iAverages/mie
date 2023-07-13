package app

import (
	"os"
	"os/signal"
	"syscall"

	"github.com/iAverage/mie/internal/bot"
	"github.com/iAverage/mie/internal/config"
	"go.uber.org/zap"
)

func Start(config *config.Config, logger *zap.SugaredLogger) {
	discord, err := bot.Create(config)
	if err != nil {
		logger.Fatal(err)
	}

	err = discord.Open()
	if err != nil {
		logger.Fatal(err)
	}

	handlers := bot.CreateHandlers(logger, config)

	discord.AddHandler(handlers.MessageCreate)

	// Wait here until CTRL-C or other term signal is received.
	logger.Info("Bot is now running.  Press CTRL-C to exit.")
	sc := make(chan os.Signal, 1)
	signal.Notify(sc, syscall.SIGINT, syscall.SIGTERM, os.Interrupt)
	<-sc

	logger.Info("Shutting down bot...")
	// Cleanly close down the Discord session.
	discord.Close()
}
