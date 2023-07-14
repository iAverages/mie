package downloader

import (
	"encoding/json"
	"fmt"
	"os/exec"

	"github.com/iAverage/mie/internal/config"
	"go.uber.org/zap"
)

type DownloaderService struct {
	logger *zap.SugaredLogger
	config *config.Config
}

type DownloadedVideo struct {
	Filename string `json:"_filename"`
}

func NewDownloaderService(logger *zap.SugaredLogger, config *config.Config) *DownloaderService {
	return &DownloaderService{
		logger: logger,
		config: config,
	}
}

func (s *DownloaderService) Download(url string) (DownloadedVideo, error) {
	s.logger.Info("Downloading video", url)
	cmd := exec.Command("yt-dlp", "-j", "--no-simulate", "-o", "%(upload_date)s-%(id)s.%(ext)s", url)
	cmd.Dir = s.config.TempDir
	out, err := cmd.Output()
	if err != nil {
		s.logger.Errorw("Error downloading video", "error", err)
		return DownloadedVideo{}, fmt.Errorf("error downloading video: %w", err)
	}

	video := DownloadedVideo{}
	json.Unmarshal(out, &video)

	s.logger.Infow("Downloaded video", video)

	return video, nil
}
