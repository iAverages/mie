package bot

import (
	"encoding/json"
	"os"
	"os/exec"

	"github.com/bwmarrin/discordgo"
	"github.com/iAverage/mie/internal/config"
	"github.com/iAverage/mie/internal/upload"
	"go.uber.org/zap"
	"mvdan.cc/xurls/v2"
)

const (
	TEMP_DIR = "/tmp"
)

type BotServices struct {
	UploadService *upload.UploadService
}

func Create(config *config.Config) (*discordgo.Session, error) {
	return discordgo.New("Bot " + config.Token)
}

type YTDLVideo struct {
	Filename string `json:"_filename"`
}

type Bot struct {
	logger *zap.SugaredLogger
	config *config.Config
}

func CreateHandlers(logger *zap.SugaredLogger, config *config.Config) *Bot {
	return &Bot{
		logger: logger,
		config: config,
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
				b.logger.Errorw("Error sending message", "error", err)
				return
			}

			cmd := exec.Command("yt-dlp", "-j", "--no-simulate", "-o", "%(upload_date)s-%(id)s.%(ext)s", url)
			cmd.Dir = TEMP_DIR
			out, err := cmd.Output()
			if err != nil {
				b.logger.Errorw("Error downloading video", "error", err)
				s.ChannelMessageEdit(m.ChannelID, message.ID, "Error downloading video")
				return
			}

			ytdlVideo := YTDLVideo{}
			json.Unmarshal(out, &ytdlVideo)

			b.logger.Infow("Downloaded video", "filename", ytdlVideo.Filename)

			text := "Uploading to CDN..."
			s.ChannelMessageEdit(m.ChannelID, message.ID, text)
			file, err := os.Open(TEMP_DIR + "/" + ytdlVideo.Filename)
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

			text = "Done!, here is the file: " + b.config.HostUrl + name
			messageEdit := &discordgo.MessageEdit{
				Content: &text,
				ID:      message.ID,
				Channel: message.ChannelID,
			}

			s.ChannelMessageEditComplex(messageEdit)
		}(_url)
	}
}
