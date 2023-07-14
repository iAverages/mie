package bot

import (
	"os"

	"github.com/bwmarrin/discordgo"
	"github.com/iAverage/mie/internal/config"
	"github.com/iAverage/mie/internal/downloader"
	"github.com/iAverage/mie/internal/upload"
	"go.uber.org/zap"
	"mvdan.cc/xurls/v2"
)

type BotServices struct {
	UploadService *upload.UploadService
}

type services struct {
	uploader   *upload.UploadService
	downloader *downloader.DownloaderService
}

type Bot struct {
	logger   *zap.SugaredLogger
	config   *config.Config
	services *services
}

func Create(config *config.Config) (*discordgo.Session, error) {
	return discordgo.New("Bot " + config.Token)
}

func CreateHandlers(logger *zap.SugaredLogger, config *config.Config) *Bot {
	return &Bot{
		logger: logger,
		config: config,
		services: &services{
			uploader:   upload.NewUploadService(logger, config),
			downloader: downloader.NewDownloaderService(logger, config),
		},
	}
}

func (b Bot) MessageCreate(s *discordgo.Session, m *discordgo.MessageCreate) {
	if m.Author.ID == s.State.User.ID {
		return
	}

	uploader := upload.NewUploadService(b.logger, b.config)

	rx, err := xurls.StrictMatchingScheme("https")
	if err != nil {
		panic(err)
	}

	urls := rx.FindAllString(m.Content, -1)
	if len(urls) == 0 {
		return
	}

	for _, _url := range urls {
		go func(url string) {
			message, err := s.ChannelMessageSend(m.ChannelID, "Downloading...")
			if err != nil {
				// probably dont have permissions to send messages
				// or discord broke, dont try to send a message since
				// one already failed
				b.logger.Errorw("Error sending message", "error", err)
				return
			}

			videoPath, err := b.services.downloader.Download(url)

			if err != nil {
				s.ChannelMessageEdit(m.ChannelID, message.ID, "Error downloading video")
				return
			}

			s.ChannelMessageEdit(m.ChannelID, message.ID, "Uploading to CDN...")

			file, err := os.Open(b.config.TempDir + "/" + videoPath.Filename)
			if err != nil {
				b.logger.Errorw("Error opening file", "error", err)
				s.ChannelMessageEdit(m.ChannelID, message.ID, "Error uploading file")
				return
			}
			defer file.Close()

			name, err := uploader.UploadFile(file)
			if err != nil {
				b.logger.Errorw("Error opening file", "error", err)
				s.ChannelMessageEdit(m.ChannelID, message.ID, "Error uploading file")
				return
			}

			text := "Done!, here is the file: " + b.config.HostUrl + name
			messageEdit := &discordgo.MessageEdit{
				Content: &text,
				ID:      message.ID,
				Channel: message.ChannelID,
			}

			s.ChannelMessageEditComplex(messageEdit)
		}(_url)
	}
}
