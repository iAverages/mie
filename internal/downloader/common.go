package downloader

type Downloader interface {
	Download(url string, outputName string) (DownloadedVideo, error)
}
