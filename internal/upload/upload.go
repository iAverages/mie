package upload

import (
	"bufio"
	"context"
	"io"
	"net/http"
	"os"
	"path/filepath"

	"github.com/iAverage/mie/internal/config"
	"github.com/minio/minio-go/v7"
	"github.com/minio/minio-go/v7/pkg/credentials"
	"go.uber.org/zap"
)

type UploadService struct {
	logger *zap.SugaredLogger
	config *config.Config
	b2     *minio.Client
}

func NewUploadService(logger *zap.SugaredLogger, config *config.Config) (*UploadService, error) {
	minioClient, err := minio.New(config.B2Url, &minio.Options{
		Creds:  credentials.NewStaticV4(config.B2ApplicationKeyId, config.B2ApplicationKey, ""),
		Secure: true,
	})
	if err != nil {
		logger.Fatalw("Error creating minio client", "error", err)
		return nil, err
	}

	return &UploadService{
		logger: logger,
		config: config,
		b2:     minioClient,
	}, nil
}

func (u *UploadService) UploadFile(file *os.File) (string, error) {
	u.logger.Infow("Uploading file", "file", file.Name())

	name := u.config.B2BucketPathPrefix + "/" + filepath.Base(file.Name())

	fileBytes, err := u.getFileBytes(file)
	if err != nil {
		u.logger.Errorw("Error getting file bytes", "error", err)
		return "", err
	}

	file.Seek(0, io.SeekStart)
	data, err := u.b2.PutObject(context.Background(), u.config.B2BucketName, name, file, int64(len(fileBytes)), minio.PutObjectOptions{
		ContentType: http.DetectContentType(fileBytes),
		NumThreads:  10,
	})

	if err != nil {
		u.logger.Errorw("Error uploading file", "error", err)
		return "", err
	}

	u.logger.Infow("Uploaded file", "data", data)

	return name, nil
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
