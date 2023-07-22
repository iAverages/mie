package downloader

import (
	"bytes"
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
	s.logger.Infow("Downloading video", "url", url)
	var outb, errb bytes.Buffer
	cmd := exec.Command(s.config.YtdlPath,
		"-j",
		"--no-simulate",
		"-o", "%(upload_date)s-%(id)s.%(ext)s",
		"--concurrent-fragments", "10",
		url)
	cmd.Dir = s.config.TempDir
	cmd.Stdout = &outb
	cmd.Stderr = &errb
	cmd.Env = []string{}

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
