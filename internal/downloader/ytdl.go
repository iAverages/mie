package downloader

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os/exec"

	"github.com/iAverage/mie/internal/config"
	"go.uber.org/zap"
)

type YtdlDownloaderService struct {
	logger *zap.SugaredLogger
	config *config.Config
}

func NewYtdlService(logger *zap.SugaredLogger, config *config.Config) *YtdlDownloaderService {
	return &YtdlDownloaderService{
		logger: logger,
		config: config,
	}
}

func (s *YtdlDownloaderService) Download(url string, outputName string) (DownloadedVideo, error) {
	s.logger.Infow("Downloading video", "url", url)

	var outb, errb bytes.Buffer
	cmd := exec.Command("/usr/local/bin/yt-dlp", "-v", "-j", "--no-simulate", "-o", "%(upload_date)s-%(id)s.%(ext)s", url)
	cmd.Dir = s.config.TempDir
	cmd.Stdout = &outb
	cmd.Stderr = &errb

	err := cmd.Run()
	if err != nil {
		s.logger.Errorw("Error downloading video", "error", err, "stderr", errb.String())
		return DownloadedVideo{}, fmt.Errorf("error downloading video: %w", err)
	}

	video := DownloadedVideo{}
	json.Unmarshal(outb.Bytes(), &video)

	s.logger.Infow("Downloaded video", "video", video)

	return video, nil
}
