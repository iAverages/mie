package upload

import (
	"bufio"
	"context"
	"io"
	"net/http"
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
	u.logger.Infow("Uploading file", "file", file.Name())
	b2Client, err := b2.NewClient(context.Background(), u.config.B2ApplicationKeyId, u.config.B2ApplicationKey)
	if err != nil {
		return "", err
	}

	bucket, err := b2Client.Bucket(context.Background(), u.config.B2BucketName)
	if err != nil {
		u.logger.Errorw("Error getting bucket", "error", err)
		return "", err
	}

	name := u.config.B2BucketPathPrefix + "/" + filepath.Base(file.Name())
	object := bucket.Object(name)
	fileBytes, err := u.getFileBytes(file)
	if err != nil {
		u.logger.Errorw("Error getting file bytes", "error", err)
		return "", err
	}

	opts := b2.WithAttrsOption(&b2.Attrs{
		ContentType: http.DetectContentType(fileBytes),
	})

	writer := object.NewWriter(context.Background(), opts)
	if _, err := io.Copy(writer, file); err != nil {
		u.logger.Errorw("Error copying file to writer", "error", err)
		return "", err
	}
	u.logger.Infow("Uploaded file", "file", name)
	return name, writer.Close()
}

func (u *UploadService) getFileBytes(f *os.File) ([]byte, error) {
	stat, err := f.Stat()
	if err != nil {
		u.logger.Errorw("Error getting file stats", "error", err)
		return nil, err
	}

	bytes := make([]byte, stat.Size())
	_, err = bufio.NewReader(f).Read(bytes)
	if err != nil {
		u.logger.Errorw("Error reading file", "error", err)
		return nil, err
	}

	return bytes, nil
}
