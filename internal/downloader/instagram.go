package downloader

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"log"
	"net/http"
	"net/url"
	"regexp"

	"github.com/iAverage/mie/internal/config"
	"go.uber.org/zap"
)

type InstagramDownloaderService struct {
	logger *zap.SugaredLogger
	config *config.Config
}

func NewInstagramService(logger *zap.SugaredLogger, config *config.Config) *InstagramDownloaderService {
	return &InstagramDownloaderService{
		logger: logger,
		config: config,
	}
}

type jsonDeserialize struct {
	ContentUrl string `json:"contentUrl"`
}

func (s *InstagramDownloaderService) Download(downloadUrl string, outputName string) (DownloadedVideo, error) {
	downloadUrl = "https://proxy.dannn.workers.dev/proxy?proxyUrl=" + url.QueryEscape(downloadUrl)
	s.logger.Infow("Downloading video", "url", downloadUrl)
	response, err := http.Get(downloadUrl)
	if err != nil {
		return DownloadedVideo{}, fmt.Errorf("error getting video page: %w", err)
	}
	defer response.Body.Close()

	body, err := ioutil.ReadAll(response.Body)
	if err != nil {
		log.Fatalln(err)
	}

	videoUrl := regexp.MustCompile(`contentUrl\":\"(.*)\",\"thumb`).FindStringSubmatch(string(body))[1]

	var jsonDeserialize jsonDeserialize
	err = json.Unmarshal([]byte(`{"contentUrl":"`+videoUrl+`"}`), &jsonDeserialize)
	if err != nil {
		return DownloadedVideo{}, fmt.Errorf("error getting video link: %w", err)
	}

	s.logger.Debugw("Downloading video with ytdl", "url", jsonDeserialize.ContentUrl)

	return NewYtdlService(s.logger, s.config).Download(jsonDeserialize.ContentUrl, "")
}
