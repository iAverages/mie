package upload

import (
	"context"
	"io"
	"os"
	"path/filepath"

	"github.com/iAverage/mie/internal/config"
	"github.com/kurin/blazer/b2"
	"go.uber.org/zap"
)

type UploadService struct {
	logger *zap.SugaredLogger
	config *config.Config
}

func NewUploadService(logger *zap.SugaredLogger, config *config.Config) *UploadService {
	return &UploadService{
		logger: logger,
		config: config,
	}
}

func (u *UploadService) UploadFile(file *os.File) (string, error) {
	b2Client, err := b2.NewClient(context.Background(), u.config.B2ApplicationKeyId, u.config.B2ApplicationKey)
	if err != nil {
		return "", err
	}

	bucket, err := b2Client.Bucket(context.Background(), u.config.B2BucketName)
	if err != nil {
		return "", err
	}

	name := u.config.B2BucketPathPrefix + "/" + filepath.Base(file.Name())
	object := bucket.Object(name)
	writer := object.NewWriter(context.Background())
	if _, err := io.Copy(writer, file); err != nil {
		return "", err
	}

	return name, writer.Close()
}
