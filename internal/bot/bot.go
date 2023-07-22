package bot

import (
	"os"
	"time"

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
	uploader, err := upload.NewUploadService(logger, config)
	if err != nil {
		logger.Fatalw("Error creating upload service", "error", err)
	}

	return &Bot{
		logger: logger,
		config: config,
		services: &services{
			uploader:   uploader,
			downloader: downloader.NewDownloaderService(logger, config),
		},
	}
}

func (b Bot) MessageCreate(s *discordgo.Session, m *discordgo.MessageCreate) {
	if m.Author.ID == s.State.User.ID {
		return
	}

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
			start := time.Now()
			message, err := s.ChannelMessageSendEmbed(m.ChannelID, &discordgo.MessageEmbed{
				Title: "Downloading...",
				Color: b.config.EmbedColor,
			})

			if err != nil {
				// probably dont have permissions to send messages
				// or discord broke, dont try to send a message since
				// one already failed
				b.logger.Errorw("Error sending message", "error", err)
				return
			}

			videoDownloadStart := time.Now()
			videoPath, err := b.services.downloader.Download(url)
			videoDownloadTime := time.Since(videoDownloadStart)

			if err != nil {
				s.ChannelMessageEditEmbed(m.ChannelID, message.ID, &discordgo.MessageEmbed{
					Title: "Error downloading video",
					Color: b.config.EmbedColor,
				})
				return
			}

			s.ChannelMessageEditEmbed(m.ChannelID, message.ID, &discordgo.MessageEmbed{
				Title: "Uploading to CDN...",
				Color: 11762810,
			})

			videoUploadStart := time.Now()
			file, err := os.Open(b.config.TempDir + "/" + videoPath.Filename)
			if err != nil {
				b.logger.Errorw("Error opening file", "error", err)
				s.ChannelMessageEditEmbed(m.ChannelID, message.ID, &discordgo.MessageEmbed{
					Title: "Error uploading video (open)",
					Color: 11762810,
				})
				return
			}
			defer file.Close()

			t := time.Now()
			b.logger.Infow("Uploading file", "file", file.Name())
			name, err := b.services.uploader.UploadFile(file)
			b.logger.Infow("Uploaded file", "duration", time.Since(t))
			if err != nil {
				b.logger.Errorw("Error uploading file", "error", err)
				s.ChannelMessageEditEmbed(m.ChannelID, message.ID, &discordgo.MessageEmbed{
					Title: "Error uploading video (b2)",
					Color: 11762810,
				})
				return
			}
			videoUploadTime := time.Since(videoUploadStart)

			content := b.config.HostUrl + name

			_, err = s.ChannelMessageEditComplex(&discordgo.MessageEdit{
				ID:      message.ID,
				Channel: message.ChannelID,
				Content: &content,
				Embed: &discordgo.MessageEmbed{
					Color: 11762810,
					Fields: []*discordgo.MessageEmbedField{
						{
							Name:   "Download Time",
							Value:  videoDownloadTime.Round(time.Millisecond).String(),
							Inline: true,
						},
						{
							Name:   "Upload Time",
							Value:  videoUploadTime.Round(time.Millisecond).String(),
							Inline: true,
						},
						{
							Name:   "Total Time",
							Value:  time.Since(start).Round(time.Millisecond).String(),
							Inline: true,
						},
					},
				},
			})

			if err != nil {
				b.logger.Errorw("Error editing message", "error", err)
				return
			}
		}(_url)
	}
}
